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

use std::cell::UnsafeCell;
use std::io::{self, IoSlice, IoSliceMut};
use std::net::{IpAddr, SocketAddr};
use std::task::{Context, Poll};
use std::time::Duration;

use g3_socket::cmsg::udp::RecvAncillaryData;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::RawSocketAddr;
#[cfg(unix)]
pub use unix::SendMsgHdr;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::RawSocketAddr;

pub trait UdpSocketExt {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>>;

    fn try_sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize>;

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
    ))]
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;
}

pub struct RecvMsgHdr<'a, const C: usize> {
    pub iov: [IoSliceMut<'a>; C],
    pub n_recv: usize,
    c_addr: UnsafeCell<RawSocketAddr>,
    dst_ip: Option<IpAddr>,
    interface_id: Option<u32>,
}

impl<const C: usize> RecvAncillaryData for RecvMsgHdr<'_, C> {
    fn set_recv_interface(&mut self, id: u32) {
        self.interface_id = Some(id);
    }

    fn set_recv_dst_addr(&mut self, addr: IpAddr) {
        self.dst_ip = Some(addr);
    }

    fn set_timestamp(&mut self, _ts: Duration) {}
}

impl<'a, const C: usize> RecvMsgHdr<'a, C> {
    pub fn new(iov: [IoSliceMut<'a>; C]) -> Self {
        RecvMsgHdr {
            iov,
            n_recv: 0,
            c_addr: UnsafeCell::new(RawSocketAddr::default()),
            dst_ip: None,
            interface_id: None,
        }
    }

    #[inline]
    pub fn dst_ip(&self) -> Option<IpAddr> {
        self.dst_ip
    }

    pub fn dst_addr(&self, local_addr: SocketAddr) -> SocketAddr {
        self.dst_ip
            .map(|ip| SocketAddr::new(ip, local_addr.port()))
            .unwrap_or(local_addr)
    }

    #[inline]
    pub fn interface_id(&self) -> Option<u32> {
        self.interface_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::ext::SocketAddrExt;
    use g3_types::net::UdpListenConfig;
    use std::future::poll_fn;
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

        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &[IoSlice::new(msg_1)], None))
            .await
            .unwrap();
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

        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &[IoSlice::new(msg_1)], Some(s_addr)))
            .await
            .unwrap();
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

        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &[IoSlice::new(msg_1)], None))
            .await
            .unwrap();
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

        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &[IoSlice::new(msg_1)], None))
            .await
            .unwrap();
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

        let nw = poll_fn(|cx| c_sock.poll_sendmsg(cx, &[IoSlice::new(msg_1)], None))
            .await
            .unwrap();
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
