/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::cell::UnsafeCell;
use std::future::Future;
use std::io::{self, IoSlice, IoSliceMut};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use quinn::udp::{RecvMeta, Transmit, UdpState};
use quinn::{AsyncTimer, AsyncUdpSocket, Runtime};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::{broadcast, oneshot};
use tokio::time::sleep_until;

use g3_io_ext::UdpSocketExt;
use g3_types::net::{Host, UpstreamAddr};

use super::{UdpInput, UdpOutput};

#[derive(Debug)]
pub struct Socks5UdpTokioRuntime {
    local_addr: SocketAddr,
    ctl_close_receiver: broadcast::Receiver<Option<Arc<io::Error>>>,
    ctl_drop_receiver: oneshot::Receiver<()>,
}

impl Drop for Socks5UdpTokioRuntime {
    fn drop(&mut self) {
        self.ctl_drop_receiver.close();
    }
}

impl Socks5UdpTokioRuntime {
    pub fn new<R>(ctl_stream: R, udp_local_addr: SocketAddr) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let (ctl_close_sender, ctl_close_receiver) = broadcast::channel(1);
        let (mut ctl_drop_notifier, ctl_drop_receiver) = oneshot::channel();
        tokio::spawn(async move {
            let mut stream = ctl_stream;
            let mut buf = [0u8; 4];

            tokio::select! {
                biased;

                r = stream.read(&mut buf) => {
                    let e = match r {
                        Ok(0) => None,
                        Ok(_) => Some(Arc::new(io::Error::new(
                            io::ErrorKind::Other,
                            "unexpected data received in the ctl connection",
                        ))),
                        Err(e) => Some(Arc::new(e)),
                    };
                    let _ = ctl_close_sender.send(e);
                }
                _ = ctl_drop_notifier.closed() => {}
            }
        });

        Socks5UdpTokioRuntime {
            local_addr: udp_local_addr,
            ctl_close_receiver,
            ctl_drop_receiver,
        }
    }
}

impl Runtime for Socks5UdpTokioRuntime {
    fn new_timer(&self, i: Instant) -> Pin<Box<dyn AsyncTimer>> {
        Box::pin(sleep_until(i.into()))
    }

    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }

    fn wrap_udp_socket(&self, t: UdpSocket) -> io::Result<Box<dyn AsyncUdpSocket>> {
        let (sender, receiver) = oneshot::channel();
        let mut ctl_close_receiver = self.ctl_close_receiver.resubscribe();
        tokio::spawn(async move {
            match ctl_close_receiver.recv().await {
                Ok(Some(e)) => sender.send(Some(io::Error::new(e.kind(), e.to_string()))),
                Ok(None) => sender.send(None),
                Err(_) => sender.send(None),
            }
        });
        let io = tokio::net::UdpSocket::from_std(t)?;
        Ok(Box::new(Socks5UdpSocket {
            io,
            local_addr: self.local_addr,
            ctl_close_receiver: UnsafeCell::new(receiver),
        }))
    }
}

#[derive(Debug)]
pub struct Socks5UdpSocket {
    io: tokio::net::UdpSocket,
    local_addr: SocketAddr,
    ctl_close_receiver: UnsafeCell<oneshot::Receiver<Option<io::Error>>>,
}

impl AsyncUdpSocket for Socks5UdpSocket {
    fn poll_send(
        &self,
        _state: &UdpState,
        cx: &mut Context,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        // logics from quinn-udp::fallback.rs
        let io = &self.io;
        let mut sent = 0;
        for transmit in transmits {
            let ups = UpstreamAddr::from(transmit.destination);
            let hdr_len = UdpOutput::calc_header_len(&ups);
            let mut buf = [0u8; 22]; // enough for ipv6
            UdpOutput::generate_header(&mut buf, &ups);

            match io.poll_sendmsg(
                cx,
                &[
                    IoSlice::new(&buf[0..hdr_len]),
                    IoSlice::new(&transmit.contents),
                ],
                None,
            ) {
                Poll::Ready(ready) => match ready {
                    Ok(_) => {
                        sent += 1;
                    }
                    // We need to report that some packets were sent in this case, so we rely on
                    // errors being either harmlessly transient (in the case of WouldBlock) or
                    // recurring on the next call.
                    Err(_) if sent != 0 => return Poll::Ready(Ok(sent)),
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            return Poll::Ready(Err(e));
                        }

                        // Other errors are ignored, since they will ususally be handled
                        // by higher level retransmits and timeouts.
                        // - PermissionDenied errors have been observed due to iptable rules.
                        //   Those are not fatal errors, since the
                        //   configuration can be dynamically changed.
                        // - Destination unreachable errors have been observed for other
                        // log_sendmsg_error(&mut self.last_send_error, e, transmit);
                        sent += 1;
                    }
                },
                Poll::Pending => {
                    return if sent == 0 {
                        Poll::Pending
                    } else {
                        Poll::Ready(Ok(sent))
                    }
                }
            }
        }
        Poll::Ready(Ok(sent))
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        // logics from quinn-udp::fallback.rs
        let ctl_close_receiver = unsafe { &mut *self.ctl_close_receiver.get() };
        match Pin::new(ctl_close_receiver).poll(cx) {
            Poll::Pending => {}
            Poll::Ready(Ok(Some(e))) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("ctl socket closed: {e:?}"),
                )));
            }
            Poll::Ready(Ok(None)) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    "ctl socket closed",
                )));
            }
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    "ctl socket closed",
                )));
            }
        }

        let Some(buf) = bufs.get_mut(0) else {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "no buf")));
        };
        let mut read_buf = ReadBuf::new(buf.as_mut());
        match self.io.poll_recv(cx, &mut read_buf) {
            Poll::Ready(res) => match res {
                Ok(_) => {
                    let mut len = read_buf.filled().len();

                    let (off, ups) = UdpInput::parse_header(buf.as_ref())
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    let addr = match ups.host() {
                        Host::Ip(ip) => SocketAddr::new(*ip, ups.port()),
                        Host::Domain(_) => {
                            // invalid reply packet, use unspecified addr instead of return error
                            let ip = match self.local_addr {
                                SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                                SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                            };
                            SocketAddr::new(ip, ups.port())
                        }
                    };
                    // TODO use IoSliceMut::advance instead of copy, then test
                    buf.copy_within(off..len, 0);
                    len -= off;

                    meta[0] = RecvMeta {
                        len,
                        stride: len,
                        addr,
                        ecn: None,
                        dst_ip: None,
                    };
                    Poll::Ready(Ok(1))
                }
                Err(err) => Poll::Ready(Err(err)),
            },
            Poll::Pending => Poll::Pending,
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local_addr)
    }

    fn may_fragment(&self) -> bool {
        false
    }
}
