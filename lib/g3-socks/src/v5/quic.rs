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
use std::net::{SocketAddr, UdpSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use std::time::Instant;

use quinn::udp::{RecvMeta, Transmit, UdpState};
use quinn::{AsyncTimer, AsyncUdpSocket, Runtime};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{broadcast, oneshot};
use tokio::time::sleep_until;

use g3_io_ext::UdpSocketExt;
use g3_types::net::Host;

use super::udp_io::{UDP_HEADER_LEN_IPV4, UDP_HEADER_LEN_IPV6};
use super::{UdpInput, UdpOutput};

#[derive(Debug)]
pub struct Socks5UdpTokioRuntime {
    quic_peer_addr: SocketAddr,
    ctl_close_receiver: broadcast::Receiver<Option<Arc<io::Error>>>,
    ctl_drop_receiver: oneshot::Receiver<()>,
    send_socks_header: SocksHeaderBuffer,
}

impl Drop for Socks5UdpTokioRuntime {
    fn drop(&mut self) {
        self.ctl_drop_receiver.close();
    }
}

impl Socks5UdpTokioRuntime {
    pub fn new<R>(ctl_stream: R, quic_peer_addr: SocketAddr) -> Self
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
                        Ok(_) => Some(Arc::new(io::Error::other("unexpected data received in the ctl connection"))),
                        Err(e) => Some(Arc::new(e)),
                    };
                    let _ = ctl_close_sender.send(e);
                }
                _ = ctl_drop_notifier.closed() => {}
            }
        });
        let send_socks_header = SocksHeaderBuffer::new_filled(quic_peer_addr);

        Socks5UdpTokioRuntime {
            quic_peer_addr,
            ctl_close_receiver,
            ctl_drop_receiver,
            send_socks_header,
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
            quic_peer_addr: self.quic_peer_addr,
            ctl_close_receiver: UnsafeCell::new(receiver),
            send_socks_header: self.send_socks_header,
            recv_socks_headers: UnsafeCell::new(vec![
                SocksHeaderBuffer::new(self.quic_peer_addr);
                4
            ]),
        }))
    }
}

#[derive(Clone, Copy, Debug)]
enum SocksHeaderBuffer {
    V4([u8; UDP_HEADER_LEN_IPV4]),
    V6([u8; UDP_HEADER_LEN_IPV6]),
}

impl SocksHeaderBuffer {
    fn new_filled(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(_) => {
                let mut buf = [0u8; UDP_HEADER_LEN_IPV4];
                UdpOutput::generate_header2(&mut buf, addr);
                SocksHeaderBuffer::V4(buf)
            }
            SocketAddr::V6(_) => {
                let mut buf = [0u8; UDP_HEADER_LEN_IPV6];
                UdpOutput::generate_header2(&mut buf, addr);
                SocksHeaderBuffer::V6(buf)
            }
        }
    }

    fn new(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(_) => SocksHeaderBuffer::V4([0u8; UDP_HEADER_LEN_IPV4]),
            SocketAddr::V6(_) => SocksHeaderBuffer::V6([0u8; UDP_HEADER_LEN_IPV6]),
        }
    }
}

impl AsRef<[u8]> for SocksHeaderBuffer {
    fn as_ref(&self) -> &[u8] {
        match self {
            SocksHeaderBuffer::V4(b) => b.as_ref(),
            SocksHeaderBuffer::V6(b) => b.as_ref(),
        }
    }
}

impl AsMut<[u8]> for SocksHeaderBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            SocksHeaderBuffer::V4(b) => b.as_mut(),
            SocksHeaderBuffer::V6(b) => b.as_mut(),
        }
    }
}

#[derive(Debug)]
pub struct Socks5UdpSocket {
    io: tokio::net::UdpSocket,
    quic_peer_addr: SocketAddr,
    ctl_close_receiver: UnsafeCell<oneshot::Receiver<Option<io::Error>>>,
    send_socks_header: SocksHeaderBuffer,
    recv_socks_headers: UnsafeCell<Vec<SocksHeaderBuffer>>,
}

impl AsyncUdpSocket for Socks5UdpSocket {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_send(
        &self,
        _state: &UdpState,
        cx: &mut Context,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        use g3_io_ext::SendMsgHdr;

        let mut msgs = Vec::with_capacity(transmits.len());

        for transmit in transmits {
            assert_eq!(self.quic_peer_addr, transmit.destination);

            msgs.push(SendMsgHdr::new(
                [
                    IoSlice::new(self.send_socks_header.as_ref()),
                    IoSlice::new(&transmit.contents),
                ],
                None,
            ))
        }

        self.io.poll_batch_sendmsg(cx, &mut msgs)
    }

    #[cfg(target_os = "macos")]
    fn poll_send(
        &self,
        _state: &UdpState,
        cx: &mut Context,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        // logics from quinn-udp::fallback.rs
        let mut sent = 0;
        for transmit in transmits {
            assert_eq!(self.quic_peer_addr, transmit.destination);

            match self.io.poll_sendmsg(
                cx,
                &[
                    IoSlice::new(self.send_socks_header.as_ref()),
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

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        use g3_io_ext::RecvMsgHdr;

        let ctl_close_receiver = unsafe { &mut *self.ctl_close_receiver.get() };
        match Pin::new(ctl_close_receiver).poll(cx) {
            Poll::Pending => {}
            Poll::Ready(Ok(Some(e))) => {
                return Poll::Ready(Err(io::Error::other(format!("ctl socket closed: {e:?}"))));
            }
            Poll::Ready(Ok(None)) => {
                return Poll::Ready(Err(io::Error::other("ctl socket closed")));
            }
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(io::Error::other("ctl socket closed")));
            }
        }

        let recv_socks_headers = unsafe { &mut *self.recv_socks_headers.get() };
        if bufs.len() > recv_socks_headers.len() {
            recv_socks_headers.resize(bufs.len(), SocksHeaderBuffer::new(self.quic_peer_addr));
        }

        let mut hdr_v = Vec::with_capacity(meta.len());
        for (b, s) in bufs.iter_mut().zip(recv_socks_headers.iter_mut()) {
            hdr_v.push(RecvMsgHdr::new([
                IoSliceMut::new(s.as_mut()),
                IoSliceMut::new(b.as_mut()),
            ]))
        }

        match ready!(self.io.poll_batch_recvmsg(cx, &mut hdr_v)) {
            Ok(count) => {
                for (h, m) in hdr_v.iter_mut().take(count).zip(meta.iter_mut()) {
                    let mut len = h.n_recv;
                    let socks_header = &h.iov[0];
                    let socks_header_len = socks_header.as_ref().len();
                    if len <= socks_header_len {
                        // ignore invalid packets
                        *m = RecvMeta {
                            len: 0,
                            addr: self.quic_peer_addr,
                            stride: 0,
                            ecn: None,
                            dst_ip: None,
                        };
                        continue;
                    }

                    let (off, ups) =
                        UdpInput::parse_header(socks_header.as_ref()).map_err(io::Error::other)?;
                    assert_eq!(socks_header_len, off);
                    let ip = match ups.host() {
                        Host::Ip(ip) => *ip,
                        Host::Domain(_) => {
                            // invalid reply packet, default to use the peer ip
                            self.quic_peer_addr.ip()
                        }
                    };
                    let port = ups.port();
                    let port = if port == 0 {
                        self.quic_peer_addr.port()
                    } else {
                        port
                    };

                    len -= off;
                    *m = RecvMeta {
                        len,
                        stride: len,
                        addr: SocketAddr::new(ip, port),
                        ecn: None,
                        dst_ip: None,
                    };
                }
                Poll::Ready(Ok(count))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    #[cfg(target_os = "macos")]
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
                return Poll::Ready(Err(io::Error::other(format!("ctl socket closed: {e:?}"))));
            }
            Poll::Ready(Ok(None)) => {
                return Poll::Ready(Err(io::Error::other("ctl socket closed")));
            }
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(io::Error::other("ctl socket closed")));
            }
        }

        let Some(buf) = bufs.get_mut(0) else {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "no buf")));
        };
        let recv_socks_headers = unsafe { &mut *self.recv_socks_headers.get() };
        let recv_socks_header = recv_socks_headers.get_mut(0).unwrap();

        let mut iov = [
            IoSliceMut::new(recv_socks_header.as_mut()),
            IoSliceMut::new(buf),
        ];

        let (mut len, _) = ready!(self.io.poll_recvmsg(cx, &mut iov))?;
        let socks_header_len = recv_socks_header.as_ref().len();
        if len <= socks_header_len {
            meta[0] = RecvMeta {
                len: 0,
                stride: 0,
                addr: self.quic_peer_addr,
                ecn: None,
                dst_ip: None,
            };
            return Poll::Ready(Ok(1));
        }

        let (off, ups) =
            UdpInput::parse_header(recv_socks_header.as_ref()).map_err(io::Error::other)?;
        assert_eq!(socks_header_len, off);
        let ip = match ups.host() {
            Host::Ip(ip) => *ip,
            Host::Domain(_) => {
                // invalid reply packet, default to use the peer ip
                self.quic_peer_addr.ip()
            }
        };
        let port = ups.port();
        let port = if port == 0 {
            self.quic_peer_addr.port()
        } else {
            port
        };

        len -= off;
        meta[0] = RecvMeta {
            len,
            stride: len,
            addr: SocketAddr::new(ip, port),
            ecn: None,
            dst_ip: None,
        };
        Poll::Ready(Ok(1))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.quic_peer_addr)
    }

    fn may_fragment(&self) -> bool {
        false
    }
}
