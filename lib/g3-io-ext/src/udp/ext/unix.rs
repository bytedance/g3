/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;
use std::os::fd::AsRawFd;
use std::task::{Context, Poll, ready};
use std::{io, ptr};

use tokio::io::Interest;
use tokio::net::UdpSocket;

use g3_io_sys::udp::{RecvAncillaryBuffer, RecvMsgHdr, SendMsgHdr};

thread_local! {
    static RECV_ANCILLARY_BUFFERS: RefCell<Vec<RecvAncillaryBuffer>> = const { RefCell::new(Vec::new()) };
}

use super::UdpSocketExt;

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &SendMsgHdr<'_, C>,
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
        let flags = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags = libc::MSG_DONTWAIT;

        let mut msghdr = unsafe { hdr.to_msghdr() };

        let raw_fd = self.as_raw_fd();
        let mut sendmsg = || {
            let r = unsafe { libc::sendmsg(raw_fd, ptr::from_mut(&mut msghdr), flags) };
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r as usize)
            }
        };

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, &mut sendmsg) {
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

    fn try_sendmsg<const C: usize>(&self, hdr: &SendMsgHdr<'_, C>) -> io::Result<usize> {
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
        let flags = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags = libc::MSG_DONTWAIT;

        let mut msghdr = unsafe { hdr.to_msghdr() };

        let raw_fd = self.as_raw_fd();

        self.try_io(Interest::WRITABLE, || {
            let r = unsafe { libc::sendmsg(raw_fd, ptr::from_mut(&mut msghdr), flags) };
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r as usize)
            }
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
                g3_io_sys::ffi::sendmsg_x(
                    raw_fd,
                    msgvec.as_mut_ptr(),
                    msgvec.len() as _,
                    flags as _,
                )
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
                    g3_io_sys::ffi::recvmsg_x(
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
    use g3_std_ext::net::SocketAddrExt;
    use g3_types::net::UdpListenConfig;
    use std::future::poll_fn;
    use std::io::{IoSlice, IoSliceMut};
    use std::net::{IpAddr, SocketAddr};
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
