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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use g3_types::net::{Host, UpstreamAddr};

use bytes::{Buf, BufMut};

use super::SocksUdpPacketError;

pub struct UdpInput {}

impl UdpInput {
    pub fn parse_header(buf: &[u8]) -> Result<(usize, UpstreamAddr), SocksUdpPacketError> {
        let len = buf.len();
        if len <= 8 {
            return Err(SocksUdpPacketError::TooSmallPacket);
        }

        if buf[0] != 0x00 || buf[1] != 0x00 {
            return Err(SocksUdpPacketError::ReservedNotZeroed);
        }

        if buf[2] != 0x00 {
            return Err(SocksUdpPacketError::FragmentNotSupported);
        }

        let (off, addr) = match buf[3] {
            0x01 => {
                const HEADER_LEN: usize = 10;
                if len <= HEADER_LEN {
                    return Err(SocksUdpPacketError::TooSmallPacket);
                }

                let mut buf = &buf[4..];
                let ip4 = Ipv4Addr::from(buf.get_u32());
                let port = buf.get_u16();
                (
                    HEADER_LEN,
                    UpstreamAddr::from_ip_and_port(IpAddr::V4(ip4), port),
                )
            }
            0x03 => {
                let domain_len = buf[4] as usize;
                let header_len = 4 + 1 + domain_len + 2;
                if len <= header_len {
                    return Err(SocksUdpPacketError::TooSmallPacket);
                }

                let domain = std::str::from_utf8(&buf[5..5 + domain_len])
                    .map_err(|_| SocksUdpPacketError::InvalidDomainString)?;
                let port_off = 5 + domain_len;
                let port = ((buf[port_off] as u16) << 8) + buf[port_off + 1] as u16;
                let addr = UpstreamAddr::from_host_str_and_port(domain, port)
                    .map_err(|_| SocksUdpPacketError::InvalidDomainString)?;
                (header_len, addr)
            }
            0x04 => {
                const HEADER_LEN: usize = 22;
                if len <= HEADER_LEN {
                    return Err(SocksUdpPacketError::TooSmallPacket);
                }

                let mut buf = &buf[4..];
                let ip6 = Ipv6Addr::from(buf.get_u128());
                let port = buf.get_u16();
                (
                    HEADER_LEN,
                    UpstreamAddr::from_ip_and_port(IpAddr::V6(ip6), port),
                )
            }
            _ => return Err(SocksUdpPacketError::InvalidAddrType),
        };

        Ok((off, addr))
    }
}

pub struct UdpOutput {}

impl UdpOutput {
    pub fn calc_header_len(upstream: &UpstreamAddr) -> usize {
        match upstream.host() {
            Host::Ip(ip) => match ip {
                IpAddr::V6(ip6) => match ip6.to_ipv4_mapped() {
                    Some(_) => 10,
                    None => 22,
                },
                IpAddr::V4(_) => 10,
            },
            Host::Domain(domain) => {
                let domain_len = domain.len().max(u8::MAX as usize) as u8;
                5 + domain_len as usize + 2
            }
        }
    }

    /// the buf len should be equal to the result of calc_header_len()
    pub fn generate_header(mut buf: &mut [u8], upstream: &UpstreamAddr) {
        buf.put_u16(0x00);
        buf.put_u8(0x00);
        match upstream.host() {
            Host::Ip(ip) => match ip {
                IpAddr::V4(ip4) => {
                    buf.put_u8(0x01);
                    buf.put_slice(&ip4.octets());
                    buf.put_u16(upstream.port());
                }
                IpAddr::V6(ip6) => match ip6.to_ipv4_mapped() {
                    Some(ip4) => {
                        buf.put_u8(0x01);
                        buf.put_slice(&ip4.octets());
                        buf.put_u16(upstream.port());
                    }
                    None => {
                        buf.put_u8(0x04);
                        buf.put_slice(&ip6.octets());
                        buf.put_u16(upstream.port());
                    }
                },
            },
            Host::Domain(domain) => {
                buf.put_u8(0x03);
                let domain_len = domain.len().max(u8::MAX as usize) as u8;
                buf.put_u8(domain_len);
                buf.put_slice(&domain.as_bytes()[0..domain_len as usize]);
                buf.put_u16(upstream.port());
            }
        }
    }
}
