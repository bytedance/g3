/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{RecvAncillaryBuffer, RecvAncillaryData};

const fn cmsg_len(length: usize) -> usize {
    unsafe { libc::CMSG_LEN(length as _) as usize }
}

const fn cmsg_space(length: usize) -> usize {
    unsafe { libc::CMSG_SPACE(length as _) as usize }
}

const CMSG_HDR_SIZE: usize = cmsg_len(0);

impl RecvAncillaryBuffer {
    #[allow(clippy::single_match)]
    pub fn parse_buf<T: RecvAncillaryData>(control_buf: &[u8], data: &mut T) -> io::Result<()> {
        let total_size = control_buf.len();
        let mut offset = 0usize;

        while offset + CMSG_HDR_SIZE <= total_size {
            let buf = &control_buf[offset..];
            let hdr = unsafe { (buf.as_ptr() as *const libc::cmsghdr).as_ref().unwrap() };
            let msg_len: usize = hdr.cmsg_len as _;
            if msg_len <= CMSG_HDR_SIZE {
                // empty record
                break;
            }
            if offset + msg_len > total_size {
                // too much payload data
                break;
            }
            offset += cmsg_space(msg_len - CMSG_HDR_SIZE);

            let payload = &buf[CMSG_HDR_SIZE..msg_len];

            match hdr.cmsg_level {
                libc::SOL_SOCKET => {}
                libc::IPPROTO_IP => match hdr.cmsg_type {
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    libc::IP_PKTINFO => {
                        if payload.len() < size_of::<libc::in_pktinfo>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct in_pktinfo",
                            ));
                        }
                        let pktinfo: &libc::in_pktinfo = unsafe {
                            (payload.as_ptr() as *const libc::in_pktinfo)
                                .as_ref()
                                .unwrap()
                        };

                        let ifindex = u32::try_from(pktinfo.ipi_ifindex).unwrap_or_default();
                        data.set_recv_interface(ifindex);
                        let ip4 = Ipv4Addr::from(u32::from_be(pktinfo.ipi_addr.s_addr));
                        data.set_recv_dst_addr(IpAddr::V4(ip4));
                    }
                    #[cfg(not(any(
                        target_os = "linux",
                        target_os = "android",
                        target_os = "freebsd",
                        target_os = "openbsd",
                        target_os = "dragonfly"
                    )))]
                    libc::IP_PKTINFO => {
                        if payload.len() < size_of::<libc::in_pktinfo>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct in_pktinfo",
                            ));
                        }
                        let pktinfo: &libc::in_pktinfo = unsafe {
                            (payload.as_ptr() as *const libc::in_pktinfo)
                                .as_ref()
                                .unwrap()
                        };

                        data.set_recv_interface(pktinfo.ipi_ifindex);
                        let ip4 = Ipv4Addr::from(u32::from_be(pktinfo.ipi_addr.s_addr));
                        data.set_recv_dst_addr(IpAddr::V4(ip4));
                    }
                    #[cfg(any(
                        target_os = "freebsd",
                        target_os = "openbsd",
                        target_os = "dragonfly"
                    ))]
                    libc::IP_RECVIF => {
                        if payload.len() < size_of::<libc::sockaddr_dl>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct sockaddr_dl",
                            ));
                        }
                        let dl_addr: &libc::sockaddr_dl = unsafe {
                            (payload.as_ptr() as *const libc::sockaddr_dl)
                                .as_ref()
                                .unwrap()
                        };
                        data.set_recv_interface(dl_addr.sdl_index as u32);
                    }
                    #[cfg(any(
                        target_os = "freebsd",
                        target_os = "openbsd",
                        target_os = "dragonfly"
                    ))]
                    libc::IP_RECVDSTADDR => {
                        if payload.len() < size_of::<libc::in_addr>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct in_addr",
                            ));
                        }
                        let ipaddr: &libc::in_addr =
                            unsafe { (payload.as_ptr() as *const libc::in_addr).as_ref().unwrap() };
                        let ip4 = Ipv4Addr::from(u32::from_be(ipaddr.s_addr));
                        data.set_recv_dst_addr(IpAddr::V4(ip4));
                    }
                    _ => {}
                },
                libc::IPPROTO_IPV6 => match hdr.cmsg_type {
                    libc::IPV6_PKTINFO => {
                        if payload.len() < size_of::<libc::in6_pktinfo>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct in6_pktinfo",
                            ));
                        }
                        let pktinfo: &libc::in6_pktinfo = unsafe {
                            (payload.as_ptr() as *const libc::in6_pktinfo)
                                .as_ref()
                                .unwrap()
                        };

                        data.set_recv_interface(pktinfo.ipi6_ifindex);
                        let ip6 = Ipv6Addr::from(pktinfo.ipi6_addr.s6_addr);
                        data.set_recv_dst_addr(IpAddr::V6(ip6));
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(())
    }
}
