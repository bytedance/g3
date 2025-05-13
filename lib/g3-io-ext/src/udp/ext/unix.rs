/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::cell::{RefCell, UnsafeCell};
use std::io::IoSlice;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::{AsFd, AsRawFd};
use std::task::{Context, Poll, ready};
use std::{io, mem, ptr};

use rustix::net::{SendAncillaryBuffer, SendFlags, sendmsg, sendmsg_addr};
use tokio::io::Interest;
use tokio::net::UdpSocket;

use g3_socket::cmsg::udp::RecvAncillaryBuffer;

thread_local! {
    static RECV_ANCILLARY_BUFFERS: RefCell<Vec<RecvAncillaryBuffer>> = const { RefCell::new(Vec::new()) };
}

use super::{RecvMsgHdr, UdpSocketExt};

#[derive(Default)]
#[repr(align(8))]
pub(super) struct RawSocketAddr {
    buf: [u8; size_of::<libc::sockaddr_in6>()],
}

impl RawSocketAddr {
    unsafe fn get_ptr_and_size(&mut self) -> (*mut libc::c_void, usize) {
        unsafe {
            let p = &*(self.buf.as_ptr() as *mut libc::sockaddr);

            let size = match p.sa_family as libc::c_int {
                libc::AF_INET => size_of::<libc::sockaddr_in>(),
                libc::AF_INET6 => size_of::<libc::sockaddr_in6>(),
                _ => self.buf.len(),
            };

            (self.buf.as_mut_ptr() as _, size)
        }
    }

    fn to_std(&self) -> Option<SocketAddr> {
        let p = unsafe { &*(self.buf.as_ptr() as *mut libc::sockaddr) };

        match p.sa_family as libc::c_int {
            libc::AF_INET => {
                let v4 = unsafe { &*(self.buf.as_ptr() as *const libc::sockaddr_in) };
                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(u32::from_be(v4.sin_addr.s_addr)),
                    u16::from_be(v4.sin_port),
                )))
            }
            libc::AF_INET6 => {
                let v6 = unsafe { &*(self.buf.as_ptr() as *const libc::sockaddr_in6) };
                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(v6.sin6_addr.s6_addr),
                    u16::from_be(v6.sin6_port),
                    u32::from_be(v6.sin6_flowinfo),
                    v6.sin6_scope_id,
                )))
            }
            _ => None,
        }
    }

    fn set_std(&mut self, addr: SocketAddr) {
        match addr {
            SocketAddr::V4(v4) => {
                let a4 = unsafe { &mut *(self.buf.as_mut_ptr() as *mut libc::sockaddr_in) };
                a4.sin_family = libc::AF_INET as _;
                a4.sin_port = u16::to_be(addr.port());
                a4.sin_addr = libc::in_addr {
                    s_addr: u32::from_ne_bytes(v4.ip().octets()),
                };
            }
            SocketAddr::V6(v6) => {
                let a6 = unsafe { &mut *(self.buf.as_mut_ptr() as *mut libc::sockaddr_in6) };
                a6.sin6_family = libc::AF_INET6 as _;
                a6.sin6_port = u16::to_be(addr.port());
                a6.sin6_addr = libc::in6_addr {
                    s6_addr: v6.ip().octets(),
                };
                a6.sin6_flowinfo = u32::to_be(v6.flowinfo());
                a6.sin6_scope_id = v6.scope_id();
            }
        }
    }
}

impl From<SocketAddr> for RawSocketAddr {
    fn from(value: SocketAddr) -> Self {
        let mut v = RawSocketAddr::default();
        v.set_std(value);
        v
    }
}

pub struct SendMsgHdr<'a, const C: usize> {
    pub(crate) iov: [IoSlice<'a>; C],
    c_addr: Option<UnsafeCell<RawSocketAddr>>,
    pub n_send: usize,
}

impl<'a, const C: usize> SendMsgHdr<'a, C> {
    pub fn new(iov: [IoSlice<'a>; C], addr: Option<SocketAddr>) -> Self {
        let c_addr = addr.map(|addr| UnsafeCell::new(RawSocketAddr::from(addr)));
        SendMsgHdr {
            iov,
            c_addr,
            n_send: 0,
        }
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self) -> libc::msghdr {
        unsafe {
            let (c_addr, c_addr_len) = match &self.c_addr {
                Some(v) => {
                    let c = &mut *v.get();
                    c.get_ptr_and_size()
                }
                None => (ptr::null_mut(), 0),
            };

            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h
        }
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    #[cfg(target_os = "macos")]
    unsafe fn to_msghdr_x(&self) -> super::macos::msghdr_x {
        unsafe {
            let mut h = mem::zeroed::<super::macos::msghdr_x>();
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h
        }
    }
}

impl<'a, const C: usize> AsRef<[IoSlice<'a>]> for SendMsgHdr<'a, C> {
    fn as_ref(&self) -> &[IoSlice<'a>] {
        self.iov.as_ref()
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
    unsafe fn to_msghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> libc::msghdr {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (c_addr, c_addr_len) = c_addr.get_ptr_and_size();

            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h.msg_control = control_buf.as_ptr() as _;
            h.msg_controllen = control_buf.len() as _;
            h
        }
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    #[cfg(target_os = "macos")]
    unsafe fn to_msghdr_x(&self, control_buf: &mut RecvAncillaryBuffer) -> super::macos::msghdr_x {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (c_addr, c_addr_len) = c_addr.get_ptr_and_size();

            let mut h = mem::zeroed::<super::macos::msghdr_x>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h.msg_control = control_buf.as_ptr() as _;
            h.msg_controllen = control_buf.len() as _;
            h
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
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "illumos",
            target_os = "solaris",
        ))]
        let flags: SendFlags = SendFlags::DONTWAIT | SendFlags::NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags: SendFlags = SendFlags::DONTWAIT;

        let fd = self.as_fd();
        let mut control = SendAncillaryBuffer::default();

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, || match target {
                Some(addr) => {
                    sendmsg_addr(fd, &addr, iov, &mut control, flags).map_err(io::Error::from)
                }
                None => sendmsg(fd, iov, &mut control, flags).map_err(io::Error::from),
            }) {
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
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "illumos",
            target_os = "solaris",
        ))]
        let flags: SendFlags = SendFlags::DONTWAIT | SendFlags::NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags: SendFlags = SendFlags::DONTWAIT;

        let fd = self.as_fd();
        let mut control = SendAncillaryBuffer::default();

        self.try_io(Interest::WRITABLE, || match target {
            Some(addr) => {
                sendmsg_addr(fd, &addr, iov, &mut control, flags).map_err(io::Error::from)
            }
            None => sendmsg(fd, iov, &mut control, flags).map_err(io::Error::from),
        })
    }

    fn poll_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &mut RecvMsgHdr<'_, C>,
    ) -> Poll<io::Result<()>> {
        RECV_ANCILLARY_BUFFERS.with_borrow_mut(|buffers| {
            if buffers.is_empty() {
                buffers.push(RecvAncillaryBuffer::default());
            }
            let control_buf = &mut buffers[0];

            let mut msghdr = unsafe { hdr.to_msghdr(control_buf) };

            let raw_fd = self.as_raw_fd();
            let mut recvmsg = || {
                let r = unsafe {
                    libc::recvmsg(raw_fd, ptr::from_mut(&mut msghdr), libc::MSG_DONTWAIT as _)
                };
                if r < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(r as usize)
                }
            };

            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, &mut recvmsg) {
                    Ok(nr) => {
                        hdr.n_recv = nr;
                        control_buf.parse(msghdr.msg_controllen as _, hdr)?;
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
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_batch_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        let mut msgvec: SmallVec<[_; 32]> = SmallVec::with_capacity(msgs.len());
        for m in msgs.iter_mut() {
            msgvec.push(libc::mmsghdr {
                msg_hdr: unsafe { m.to_msghdr() },
                msg_len: 0,
            });
        }

        let raw_fd = self.as_raw_fd();
        let flags = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
        let mut sendmmsg = || {
            let r = unsafe {
                libc::sendmmsg(raw_fd, msgvec.as_mut_ptr(), msgvec.len() as _, flags as _)
            };
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r as usize)
            }
        };

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, &mut sendmmsg) {
                Ok(count) => {
                    for (m, h) in msgs.iter_mut().take(count).zip(msgvec) {
                        m.n_send = h.msg_len as usize;
                    }
                    return Poll::Ready(Ok(count));
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
    }

    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        let mut msgvec: SmallVec<[_; 32]> = SmallVec::with_capacity(msgs.len());
        for m in msgs.iter_mut() {
            msgvec.push(unsafe { m.to_msghdr_x() });
        }

        let raw_fd = self.as_raw_fd();
        let flags = libc::MSG_DONTWAIT;
        let mut sendmsg_x = || {
            let r = unsafe {
                super::macos::sendmsg_x(raw_fd, msgvec.as_mut_ptr(), msgvec.len() as _, flags as _)
            };
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r as usize)
            }
        };

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, &mut sendmsg_x) {
                Ok(count) => {
                    for m in msgs.iter_mut().take(count) {
                        m.n_send = m.iov.iter().map(|iov| iov.len()).sum();
                    }
                    return Poll::Ready(Ok(count));
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
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        RECV_ANCILLARY_BUFFERS.with_borrow_mut(|buffers| {
            if buffers.len() < hdr_v.len() {
                buffers.resize_with(hdr_v.len(), RecvAncillaryBuffer::default);
            }

            let mut msgvec: SmallVec<[_; 32]> = SmallVec::with_capacity(hdr_v.len());
            for (i, m) in hdr_v.iter_mut().enumerate() {
                let control_buf = &mut buffers[i];
                msgvec.push(libc::mmsghdr {
                    msg_hdr: unsafe { m.to_msghdr(control_buf) },
                    msg_len: 0,
                });
            }

            let raw_fd = self.as_raw_fd();
            let mut recvmmsg = || {
                let r = unsafe {
                    libc::recvmmsg(
                        raw_fd,
                        msgvec.as_mut_ptr(),
                        msgvec.len() as _,
                        libc::MSG_DONTWAIT as _,
                        ptr::null_mut(),
                    )
                };
                if r < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(r as usize)
                }
            };

            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, &mut recvmmsg) {
                    Ok(count) => {
                        for (m, h) in hdr_v.iter_mut().take(count).zip(msgvec) {
                            m.n_recv = h.msg_len as usize;
                            if h.msg_hdr.msg_control.is_null() {
                                continue;
                            }
                            let control_buf = unsafe {
                                std::slice::from_raw_parts(
                                    h.msg_hdr.msg_control as *const u8,
                                    h.msg_hdr.msg_controllen as _,
                                )
                            };
                            RecvAncillaryBuffer::parse_buf(control_buf, m)?;
                        }
                        return Poll::Ready(Ok(count));
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
    }

    #[cfg(target_os = "macos")]
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        use smallvec::SmallVec;

        RECV_ANCILLARY_BUFFERS.with_borrow_mut(|buffers| {
            if buffers.len() < hdr_v.len() {
                buffers.resize_with(hdr_v.len(), RecvAncillaryBuffer::default);
            }

            let mut msgvec: SmallVec<[_; 32]> = SmallVec::with_capacity(hdr_v.len());
            for (i, m) in hdr_v.iter_mut().enumerate() {
                let control_buf = &mut buffers[i];
                msgvec.push(unsafe { m.to_msghdr_x(control_buf) });
            }

            let raw_fd = self.as_raw_fd();
            let mut recvmsg_x = || {
                let r = unsafe {
                    super::macos::recvmsg_x(
                        raw_fd,
                        msgvec.as_mut_ptr(),
                        msgvec.len() as _,
                        libc::MSG_DONTWAIT as _,
                    )
                };
                if r < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(r as usize)
                }
            };

            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, &mut recvmsg_x) {
                    Ok(count) => {
                        for (m, h) in hdr_v.iter_mut().take(count).zip(msgvec) {
                            m.n_recv = h.msg_datalen;
                            if h.msg_control.is_null() {
                                continue;
                            }
                            let control_buf = unsafe {
                                std::slice::from_raw_parts(
                                    h.msg_control as *const u8,
                                    h.msg_controllen as usize,
                                )
                            };
                            RecvAncillaryBuffer::parse_buf(control_buf, m)?;
                        }
                        return Poll::Ready(Ok(count));
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::ext::SocketAddrExt;
    use g3_types::net::UdpListenConfig;
    use std::future::poll_fn;
    use std::io::IoSliceMut;
    use std::net::IpAddr;
    use std::str::FromStr;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    #[tokio::test]
    async fn batch_msg_connect() {
        let s_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let s_addr = s_sock.local_addr().unwrap();

        let c_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();
        c_sock.connect(&s_addr).await.unwrap();

        let msg_1 = b"abcd";
        let msg_2 = b"test";

        let mut msgs = [
            SendMsgHdr::new([IoSlice::new(msg_1)], None),
            SendMsgHdr::new([IoSlice::new(msg_2)], None),
        ];

        #[cfg(not(target_os = "macos"))]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg(cx, &mut msgs))
            .await
            .unwrap();
        #[cfg(target_os = "macos")]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg_x(cx, &mut msgs))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(msgs[0].n_send, msg_1.len());
        assert_eq!(msgs[1].n_send, msg_2.len());

        let mut recv_msg1 = [0u8; 16];
        let mut recv_msg2 = [0u8; 16];
        let mut hdr_v = [
            RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]),
            RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg2)]),
        ];
        let count = poll_fn(|cx| s_sock.poll_batch_recvmsg(cx, &mut hdr_v))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(hdr_v[0].n_recv, msg_1.len());
        assert_eq!(hdr_v[0].src_addr(), Some(c_addr));
        assert_eq!(hdr_v[1].n_recv, msg_2.len());
        assert_eq!(hdr_v[1].src_addr(), Some(c_addr));

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
        assert_eq!(&recv_msg2[..msg_2.len()], msg_2);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    #[tokio::test]
    async fn batch_msg_no_connect() {
        let s_sock = UdpSocket::bind("[::1]:0").await.unwrap();
        let s_addr = s_sock.local_addr().unwrap();

        let c_sock = UdpSocket::bind("[::1]:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();

        let msg_1 = b"abcd";
        let msg_2 = b"test";

        let mut msgs = [
            SendMsgHdr::new([IoSlice::new(msg_1)], Some(s_addr)),
            SendMsgHdr::new([IoSlice::new(msg_2)], Some(s_addr)),
        ];

        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg(cx, &mut msgs))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(msgs[0].n_send, msg_1.len());
        assert_eq!(msgs[1].n_send, msg_2.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr_v = [RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)])];
        let count = poll_fn(|cx| s_sock.poll_batch_recvmsg(cx, &mut hdr_v))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(hdr_v[0].n_recv, msg_1.len());
        assert_eq!(hdr_v[0].src_addr(), Some(c_addr));

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);

        let mut recv_msg2 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg2)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_2.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(&recv_msg2[..msg_2.len()], msg_2);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    #[tokio::test]
    async fn recv_ancillary_v4() {
        let listen_config = UdpListenConfig::new(SocketAddr::from_str("0.0.0.0:0").unwrap());
        let s_sock = g3_socket::udp::new_std_bind_listen(&listen_config).unwrap();
        let s_sock = UdpSocket::from_std(s_sock).unwrap();
        let s_addr = s_sock.local_addr().unwrap();
        assert!(s_addr.ip().is_unspecified());
        assert_ne!(s_addr.port(), 0);
        let target_s_addr = SocketAddr::new(IpAddr::from_str("127.0.0.1").unwrap(), s_addr.port());

        let c_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();
        c_sock.connect(&target_s_addr).await.unwrap();

        let msg_1 = b"abcd";
        let msg_2 = b"test";

        let mut msgs = [
            SendMsgHdr::new([IoSlice::new(msg_1)], None),
            SendMsgHdr::new([IoSlice::new(msg_2)], None),
        ];

        #[cfg(not(target_os = "macos"))]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg(cx, &mut msgs))
            .await
            .unwrap();
        #[cfg(target_os = "macos")]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg_x(cx, &mut msgs))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(msgs[0].n_send, msg_1.len());
        assert_eq!(msgs[1].n_send, msg_2.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr_v = [RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)])];
        let count = poll_fn(|cx| s_sock.poll_batch_recvmsg(cx, &mut hdr_v))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(hdr_v[0].n_recv, msg_1.len());
        assert_eq!(hdr_v[0].src_addr(), Some(c_addr));
        assert_eq!(hdr_v[0].dst_addr(s_addr), target_s_addr);
        assert!(hdr_v[0].interface_id().is_some());

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);

        let mut recv_msg2 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg2)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_2.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(hdr.dst_addr(s_addr), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg2[..msg_2.len()], msg_2);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    #[tokio::test]
    async fn recv_ancillary_v6() {
        let mut listen_config = UdpListenConfig::new(SocketAddr::from_str("[::]:0").unwrap());
        #[cfg(not(target_os = "openbsd"))]
        listen_config.set_ipv6_only(true);
        let s_sock = g3_socket::udp::new_std_bind_listen(&listen_config).unwrap();
        let s_sock = UdpSocket::from_std(s_sock).unwrap();
        let s_addr = s_sock.local_addr().unwrap();
        assert!(s_addr.ip().is_unspecified());
        assert_ne!(s_addr.port(), 0);
        let target_s_addr = SocketAddr::new(IpAddr::from_str("::1").unwrap(), s_addr.port());

        let c_sock = UdpSocket::bind("[::1]:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();
        c_sock.connect(&target_s_addr).await.unwrap();

        let msg_1 = b"abcd";
        let msg_2 = b"test";

        let mut msgs = [
            SendMsgHdr::new([IoSlice::new(msg_1)], None),
            SendMsgHdr::new([IoSlice::new(msg_2)], None),
        ];

        #[cfg(not(target_os = "macos"))]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg(cx, &mut msgs))
            .await
            .unwrap();
        #[cfg(target_os = "macos")]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg_x(cx, &mut msgs))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(msgs[0].n_send, msg_1.len());
        assert_eq!(msgs[1].n_send, msg_2.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr_v = [RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)])];
        let count = poll_fn(|cx| s_sock.poll_batch_recvmsg(cx, &mut hdr_v))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(hdr_v[0].n_recv, msg_1.len());
        assert_eq!(hdr_v[0].src_addr(), Some(c_addr));
        assert_eq!(hdr_v[0].dst_addr(s_addr), target_s_addr);
        assert!(hdr_v[0].interface_id().is_some());

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);

        let mut recv_msg2 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg2)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_2.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(hdr.dst_addr(s_addr), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg2[..msg_2.len()], msg_2);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    #[tokio::test]
    async fn recv_ancillary_mapped_v4() {
        let mut listen_config = UdpListenConfig::new(SocketAddr::from_str("[::]:0").unwrap());
        listen_config.set_ipv6_only(false);
        let s_sock = g3_socket::udp::new_std_bind_listen(&listen_config).unwrap();
        let s_sock = UdpSocket::from_std(s_sock).unwrap();
        let s_addr = s_sock.local_addr().unwrap();
        assert!(s_addr.ip().is_unspecified());
        assert_ne!(s_addr.port(), 0);
        let target_s_addr = SocketAddr::new(IpAddr::from_str("127.0.0.1").unwrap(), s_addr.port());

        let c_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();
        let expect_c_addr =
            SocketAddr::new(IpAddr::from_str("::ffff:127.0.0.1").unwrap(), c_addr.port());
        c_sock.connect(&target_s_addr).await.unwrap();

        let msg_1 = b"abcd";
        let msg_2 = b"test";

        let mut msgs = [
            SendMsgHdr::new([IoSlice::new(msg_1)], None),
            SendMsgHdr::new([IoSlice::new(msg_2)], None),
        ];

        #[cfg(not(target_os = "macos"))]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg(cx, &mut msgs))
            .await
            .unwrap();
        #[cfg(target_os = "macos")]
        let count = poll_fn(|cx| c_sock.poll_batch_sendmsg_x(cx, &mut msgs))
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(msgs[0].n_send, msg_1.len());
        assert_eq!(msgs[1].n_send, msg_2.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr_v = [RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)])];
        let count = poll_fn(|cx| s_sock.poll_batch_recvmsg(cx, &mut hdr_v))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(hdr_v[0].n_recv, msg_1.len());
        assert_eq!(hdr_v[0].src_addr(), Some(expect_c_addr));
        assert_eq!(hdr_v[0].dst_addr(s_addr).to_canonical(), target_s_addr);
        assert!(hdr_v[0].interface_id().is_some());

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);

        let mut recv_msg2 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg2)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_2.len());
        assert_eq!(hdr.src_addr(), Some(expect_c_addr));
        assert_eq!(hdr.dst_addr(s_addr).to_canonical(), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg2[..msg_2.len()], msg_2);
    }
}
