/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;
use std::io::IoSlice;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::windows::io::AsRawSocket;
use std::task::{Context, Poll, ready};
use std::{io, ptr};

use once_cell::unsync::OnceCell;
use tokio::io::Interest;
use tokio::net::UdpSocket;
use windows_sys::Win32::Networking::WinSock;

use g3_socket::RawSocket;
use g3_socket::cmsg::udp::RecvAncillaryBuffer;

use super::{RecvMsgHdr, UdpSocketExt};

thread_local! {
    static RECV_ANCILLARY_BUFFER: RefCell<RecvAncillaryBuffer> = const { RefCell::new(RecvAncillaryBuffer::new()) };
    static WSARECVMSG_PTR: OnceCell<WinSock::LPFN_WSARECVMSG> = const { OnceCell::new() };
}

#[derive(Default)]
pub(super) struct RawSocketAddr {
    buf: [u8; size_of::<WinSock::SOCKADDR_IN6>()],
}

impl RawSocketAddr {
    unsafe fn get_ptr_and_size(&mut self) -> (*mut WinSock::SOCKADDR, i32) {
        unsafe {
            let p = &*(self.buf.as_ptr() as *mut WinSock::SOCKADDR);

            let size = match p.sa_family {
                WinSock::AF_INET => size_of::<WinSock::SOCKADDR_IN>(),
                WinSock::AF_INET6 => size_of::<WinSock::SOCKADDR_IN6>(),
                _ => self.buf.len(),
            };

            (self.buf.as_mut_ptr() as _, size as i32)
        }
    }

    fn to_std(&self) -> Option<SocketAddr> {
        let p = unsafe { &*(self.buf.as_ptr() as *mut WinSock::SOCKADDR) };

        match p.sa_family {
            WinSock::AF_INET => {
                let v4 = unsafe { &*(self.buf.as_ptr() as *const WinSock::SOCKADDR_IN) };
                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(u32::from_be(unsafe { v4.sin_addr.S_un.S_addr })),
                    u16::from_be(v4.sin_port),
                )))
            }
            WinSock::AF_INET6 => {
                let v6 = unsafe { &*(self.buf.as_ptr() as *const WinSock::SOCKADDR_IN6) };
                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(unsafe { v6.sin6_addr.u.Byte }),
                    u16::from_be(v6.sin6_port),
                    u32::from_be(v6.sin6_flowinfo),
                    unsafe { v6.Anonymous.sin6_scope_id },
                )))
            }
            _ => None,
        }
    }
}

impl<const C: usize> RecvMsgHdr<'_, C> {
    pub fn src_addr(&self) -> Option<SocketAddr> {
        let c_addr = unsafe { &*self.c_addr.get() };
        c_addr.to_std()
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    unsafe fn to_msghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> WinSock::WSAMSG {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (name, namelen) = c_addr.get_ptr_and_size();

            WinSock::WSAMSG {
                name,
                namelen,
                lpBuffers: self.iov.as_ptr() as _,
                dwBufferCount: C as _,
                Control: WinSock::WSABUF {
                    len: control_buf.len() as _,
                    buf: control_buf.as_ptr() as _,
                },
                dwFlags: 0,
            }
        }
    }
}

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>> {
        let socket = RawSocket::from(self);

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, || socket.sendmsg(iov, target)) {
                Ok(res) => return Poll::Ready(Ok(res)),
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn try_sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize> {
        let socket = RawSocket::from(self);

        self.try_io(Interest::WRITABLE, || socket.sendmsg(iov, target))
    }

    fn poll_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &mut RecvMsgHdr<'_, C>,
    ) -> Poll<io::Result<()>> {
        WSARECVMSG_PTR.with(|v| {
            let wsa_recvmsg_ptr = v.get_or_try_init(|| get_wsa_recvmsg_ptr(self))?;

            let Some(wsa_recvmsg_ptr) = wsa_recvmsg_ptr else {
                return Poll::Ready(Err(io::Error::other(
                    "WSARECVMSG function is not available",
                )));
            };

            RECV_ANCILLARY_BUFFER.with_borrow_mut(|control_buf| {
                let mut msghdr = unsafe { hdr.to_msghdr(control_buf) };

                let raw_fd = self.as_raw_socket() as usize;
                let mut recvmsg = || {
                    let mut len = 0;
                    let r = unsafe {
                        (wsa_recvmsg_ptr)(
                            raw_fd,
                            ptr::from_mut(&mut msghdr),
                            ptr::from_mut(&mut len),
                            ptr::null_mut(),
                            None,
                        )
                    };
                    if r != 0 {
                        Err(io::Error::last_os_error())
                    } else {
                        Ok(len as usize)
                    }
                };

                loop {
                    ready!(self.poll_recv_ready(cx))?;
                    match self.try_io(Interest::READABLE, &mut recvmsg) {
                        Ok(nr) => {
                            hdr.n_recv = nr;
                            control_buf.parse(msghdr.Control.len as _, hdr)?;
                            return Poll::Ready(Ok(()));
                        }
                        Err(e) => {
                            if e.kind() == io::ErrorKind::WouldBlock {
                                continue;
                            } else {
                                return Poll::Ready(Err(e));
                            }
                        }
                    }
                }
            })
        })
    }
}

fn get_wsa_recvmsg_ptr<T: AsRawSocket>(socket: &T) -> io::Result<WinSock::LPFN_WSARECVMSG> {
    let guid = WinSock::WSAID_WSARECVMSG;
    let mut wsa_recvmsg_ptr = None;
    let mut len = 0;

    // Safety: Option handles the NULL pointer with a None value
    let rc = unsafe {
        WinSock::WSAIoctl(
            socket.as_raw_socket() as _,
            WinSock::SIO_GET_EXTENSION_FUNCTION_POINTER,
            &guid as *const _ as *const _,
            size_of_val(&guid) as u32,
            &mut wsa_recvmsg_ptr as *mut _ as *mut _,
            size_of_val(&wsa_recvmsg_ptr) as u32,
            &mut len,
            ptr::null_mut(),
            None,
        )
    };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    } else if len as usize != size_of::<WinSock::LPFN_WSARECVMSG>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid WSARecvMsg function pointer: size mismatch",
        ));
    }

    Ok(wsa_recvmsg_ptr)
}
