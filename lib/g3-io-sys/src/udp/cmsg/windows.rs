/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::{io, mem};

use windows_sys::Win32::Networking::WinSock;

use super::{RecvAncillaryBuffer, RecvAncillaryData};

const fn cmsg_align(len: usize) -> usize {
    (len + mem::align_of::<usize>() - 1) & !(mem::align_of::<usize>() - 1)
}

const fn cmsg_len(length: usize) -> usize {
    cmsg_align(mem::size_of::<WinSock::CMSGHDR>()) + length
}

const fn cmsg_space(length: usize) -> usize {
    cmsg_align(mem::size_of::<WinSock::CMSGHDR>()) + cmsg_align(length)
}

const CMSG_HDR_SIZE: usize = cmsg_len(0);

impl RecvAncillaryBuffer {
    #[allow(clippy::single_match)]
    pub fn parse_buf<T: RecvAncillaryData>(control_buf: &[u8], data: &mut T) -> io::Result<()> {
        let total_size = control_buf.len();
        let mut offset = 0usize;

        while offset + CMSG_HDR_SIZE <= total_size {
            let buf = &control_buf[offset..];
            let hdr = unsafe { (buf.as_ptr() as *const WinSock::CMSGHDR).as_ref().unwrap() };
            if hdr.cmsg_len <= CMSG_HDR_SIZE {
                // empty record
                break;
            }
            if offset + hdr.cmsg_len > total_size {
                // too much payload data
                break;
            }
            offset += cmsg_space(hdr.cmsg_len - CMSG_HDR_SIZE);

            let payload = &buf[CMSG_HDR_SIZE..hdr.cmsg_len];

            match hdr.cmsg_level {
                WinSock::SOL_SOCKET => {}
                WinSock::IPPROTO_IP => match hdr.cmsg_type {
                    WinSock::IP_PKTINFO => {
                        if payload.len() < size_of::<WinSock::IN_PKTINFO>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct IN_PKTINFO",
                            ));
                        }
                        let pktinfo: &WinSock::IN_PKTINFO = unsafe {
                            (payload.as_ptr() as *const WinSock::IN_PKTINFO)
                                .as_ref()
                                .unwrap()
                        };

                        data.set_recv_interface(pktinfo.ipi_ifindex);
                        let ip4 =
                            Ipv4Addr::from(u32::from_be(unsafe { pktinfo.ipi_addr.S_un.S_addr }));
                        data.set_recv_dst_addr(IpAddr::V4(ip4));
                    }
                    _ => {}
                },
                WinSock::IPPROTO_IPV6 => match hdr.cmsg_type {
                    WinSock::IPV6_PKTINFO => {
                        if payload.len() < size_of::<WinSock::IN6_PKTINFO>() {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no enough msg data for struct IN6_PKTINFO",
                            ));
                        }
                        let pktinfo: &WinSock::IN6_PKTINFO = unsafe {
                            (payload.as_ptr() as *const WinSock::IN6_PKTINFO)
                                .as_ref()
                                .unwrap()
                        };

                        data.set_recv_interface(pktinfo.ipi6_ifindex);
                        let ip6 = Ipv6Addr::from(unsafe { pktinfo.ipi6_addr.u.Byte });
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
