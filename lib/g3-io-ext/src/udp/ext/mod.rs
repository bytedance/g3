/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::task::{Context, Poll};

use g3_io_sys::udp::{RecvMsgHdr, SendMsgHdr};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

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

#[cfg(test)]
mod tests {
    use super::*;
    use g3_std_ext::net::SocketAddrExt;
    use g3_types::net::UdpListenConfig;
    use std::future::poll_fn;
    use std::io::{IoSlice, IoSliceMut};
    use std::net::{IpAddr, SocketAddr};
    use std::str::FromStr;
    use tokio::net::UdpSocket;

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
}
