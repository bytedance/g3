/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;
use std::io;
use std::task::{Context, Poll, ready};

use tokio::io::Interest;
use tokio::net::UdpSocket;

use g3_io_sys::udp::{RecvAncillaryBuffer, RecvMsgHdr, SendMsgHdr, recvmsg, sendmsg};

thread_local! {
    static RECV_ANCILLARY_BUFFER: RefCell<RecvAncillaryBuffer> = const { RefCell::new(RecvAncillaryBuffer::new()) };
}

pub trait UdpSocketExt {
    fn poll_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &SendMsgHdr<'_, C>,
    ) -> Poll<io::Result<usize>>;

    fn try_sendmsg<const C: usize>(&self, hdr: &SendMsgHdr<'_, C>) -> io::Result<usize>;

    fn poll_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &mut RecvMsgHdr<'_, C>,
    ) -> Poll<io::Result<()>>;

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
    ) -> Poll<io::Result<usize>>;

    /// Do a batch sendmsg via macOS private method sendmsg_x
    ///
    /// Only work for connected socket
    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;
}

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &SendMsgHdr<'_, C>,
    ) -> Poll<io::Result<usize>> {
        let mut msghdr = unsafe { hdr.to_msghdr() };
        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, || sendmsg(self, &mut msghdr)) {
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
        let mut msghdr = unsafe { hdr.to_msghdr() };
        self.try_io(Interest::WRITABLE, || sendmsg(self, &mut msghdr))
    }

    fn poll_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &mut RecvMsgHdr<'_, C>,
    ) -> Poll<io::Result<()>> {
        RECV_ANCILLARY_BUFFER.with_borrow_mut(|control_buf| {
            let mut msghdr = unsafe { hdr.to_msghdr(control_buf) };
            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, || recvmsg(self, &mut msghdr)) {
                    Ok(nr) => {
                        hdr.n_recv = nr;
                        control_buf.parse_msg(msghdr, hdr)?;
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
        g3_io_sys::udp::with_sendmmsg_buf(msgs, |msgs, msgvec| {
            loop {
                ready!(self.poll_send_ready(cx))?;
                match self.try_io(Interest::WRITABLE, || {
                    g3_io_sys::udp::sendmmsg(self, msgvec)
                }) {
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
        })
    }

    #[cfg(target_os = "macos")]
    fn poll_batch_sendmsg_x<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        g3_io_sys::udp::with_sendmsg_x_buf(msgs, |msgs, msgvec| {
            loop {
                ready!(self.poll_send_ready(cx))?;
                match self.try_io(Interest::WRITABLE, || {
                    g3_io_sys::udp::sendmsg_x(self, msgvec)
                }) {
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
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>> {
        g3_io_sys::udp::with_recvmmsg_buf(hdr_v, |hdr_v, msgvec| {
            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, || {
                    g3_io_sys::udp::recvmmsg(self, msgvec)
                }) {
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
        g3_io_sys::udp::with_recvmsg_x_buf(hdr_v, |hdr_v, msgvec| {
            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, || {
                    g3_io_sys::udp::recvmsg_x(self, msgvec)
                }) {
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

    #[tokio::test]
    async fn msg_connect() {
        let s_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let s_addr = s_sock.local_addr().unwrap();

        let c_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();
        c_sock.connect(&s_addr).await.unwrap();

        let msg_1 = b"abcd";

        let hdr = SendMsgHdr::new([IoSlice::new(msg_1)], None);
        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &hdr)).await.unwrap();
        assert_eq!(nw, msg_1.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_1.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));

        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
    }

    #[tokio::test]
    async fn msg_no_connect() {
        let s_sock = UdpSocket::bind("[::1]:0").await.unwrap();
        let s_addr = s_sock.local_addr().unwrap();

        let c_sock = UdpSocket::bind("[::1]:0").await.unwrap();
        let c_addr = c_sock.local_addr().unwrap();

        let msg_1 = b"abcd";

        let hdr = SendMsgHdr::new([IoSlice::new(msg_1)], Some(s_addr));
        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &hdr)).await.unwrap();
        assert_eq!(nw, msg_1.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_1.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
    }

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

        let hdr = SendMsgHdr::new([IoSlice::new(msg_1)], None);
        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &hdr)).await.unwrap();
        assert_eq!(nw, msg_1.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_1.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(hdr.dst_addr(s_addr), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
    }

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

        let hdr = SendMsgHdr::new([IoSlice::new(msg_1)], None);
        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &hdr)).await.unwrap();
        assert_eq!(nw, msg_1.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_1.len());
        assert_eq!(hdr.src_addr(), Some(c_addr));
        assert_eq!(hdr.dst_addr(s_addr), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
    }

    #[cfg(not(target_os = "openbsd"))]
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

        let hdr = SendMsgHdr::new([IoSlice::new(msg_1)], None);
        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &hdr)).await.unwrap();
        assert_eq!(nw, msg_1.len());

        let mut recv_msg1 = [0u8; 16];
        let mut hdr = RecvMsgHdr::new([IoSliceMut::new(&mut recv_msg1)]);
        poll_fn(|cx| s_sock.poll_recvmsg(cx, &mut hdr))
            .await
            .unwrap();
        assert_eq!(hdr.n_recv, msg_1.len());
        assert_eq!(hdr.src_addr(), Some(expect_c_addr));
        assert_eq!(hdr.dst_addr(s_addr).to_canonical(), target_s_addr);
        assert!(hdr.interface_id().is_some());
        assert_eq!(&recv_msg1[..msg_1.len()], msg_1);
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
    async fn batch_recv_ancillary_v4() {
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
    async fn batch_recv_ancillary_v6() {
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
    async fn batch_recv_ancillary_mapped_v4() {
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
