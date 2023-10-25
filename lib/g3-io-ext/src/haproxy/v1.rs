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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use super::{ProxyAddr, ProxyProtocolReadError};

const PROXY_DATA_V1_MAX_LEN: usize = 108;

const COMMON_DATA: &[u8] = &[b'P', b'R', b'O', b'X', b'Y', b' '];

pub struct ProxyProtocolV1Reader {
    timeout: Duration,
    data_buf: [u8; PROXY_DATA_V1_MAX_LEN],
}

impl ProxyProtocolV1Reader {
    pub fn new(timeout: Duration) -> Self {
        ProxyProtocolV1Reader {
            timeout,
            data_buf: [0u8; PROXY_DATA_V1_MAX_LEN],
        }
    }

    pub async fn read_proxy_protocol_v1_for_tcp(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<Option<ProxyAddr>, ProxyProtocolReadError> {
        match tokio::time::timeout(self.timeout, self.peek_line(stream)).await {
            Ok(Ok(l)) => self.parse_buf(&self.data_buf[0..l]),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ProxyProtocolReadError::ReadTimeout),
        }
    }

    fn parse_buf(&self, data: &[u8]) -> Result<Option<ProxyAddr>, ProxyProtocolReadError> {
        let mut iter = data[COMMON_DATA.len()..].split(|c| *c == b' ');
        let family = iter
            .next()
            .ok_or(ProxyProtocolReadError::InvalidFamily(0x00))?;
        let family_c;
        match family.len() {
            4 => {
                if !family.starts_with(b"TCP") {
                    return Err(ProxyProtocolReadError::InvalidFamily(0x00));
                }
                family_c = family[3];
            }
            7 => {
                return if family == b"UNKNOWN" {
                    Ok(None)
                } else {
                    Err(ProxyProtocolReadError::InvalidFamily(0x00))
                }
            }
            _ => {
                return Err(ProxyProtocolReadError::InvalidFamily(0x00));
            }
        }

        let src_ip = iter.next().ok_or(ProxyProtocolReadError::InvalidSrcAddr)?;
        let src_ip =
            std::str::from_utf8(src_ip).map_err(|_| ProxyProtocolReadError::InvalidSrcAddr)?;

        let dst_ip = iter.next().ok_or(ProxyProtocolReadError::InvalidDstAddr)?;
        let dst_ip =
            std::str::from_utf8(dst_ip).map_err(|_| ProxyProtocolReadError::InvalidDstAddr)?;

        let src_port = iter.next().ok_or(ProxyProtocolReadError::InvalidSrcAddr)?;
        let src_port =
            std::str::from_utf8(src_port).map_err(|_| ProxyProtocolReadError::InvalidSrcAddr)?;

        let dst_port = iter.next().ok_or(ProxyProtocolReadError::InvalidDstAddr)?;
        let dst_port =
            std::str::from_utf8(dst_port).map_err(|_| ProxyProtocolReadError::InvalidDstAddr)?;

        let (src_ip, dst_ip) = match family_c {
            b'4' => {
                let src_addr = Ipv4Addr::from_str(src_ip)
                    .map_err(|_| ProxyProtocolReadError::InvalidSrcAddr)?;
                let dst_addr = Ipv4Addr::from_str(dst_ip)
                    .map_err(|_| ProxyProtocolReadError::InvalidDstAddr)?;
                (IpAddr::V4(src_addr), IpAddr::V4(dst_addr))
            }
            b'6' => {
                let src_addr = Ipv6Addr::from_str(src_ip)
                    .map_err(|_| ProxyProtocolReadError::InvalidSrcAddr)?;
                let dst_addr = Ipv6Addr::from_str(dst_ip)
                    .map_err(|_| ProxyProtocolReadError::InvalidDstAddr)?;
                (IpAddr::V6(src_addr), IpAddr::V6(dst_addr))
            }
            c => return Err(ProxyProtocolReadError::InvalidFamily(c)),
        };

        let src_port =
            u16::from_str(src_port).map_err(|_| ProxyProtocolReadError::InvalidSrcAddr)?;
        let dst_port = u16::from_str(dst_port.trim_end())
            .map_err(|_| ProxyProtocolReadError::InvalidDstAddr)?;

        Ok(Some(ProxyAddr {
            src_addr: SocketAddr::new(src_ip, src_port),
            dst_addr: SocketAddr::new(dst_ip, dst_port),
        }))
    }

    async fn peek_line(&mut self, stream: &mut TcpStream) -> Result<usize, ProxyProtocolReadError> {
        let mut offset = 0usize;
        let mut buf = [0u8; PROXY_DATA_V1_MAX_LEN];
        loop {
            let len = stream.peek(&mut buf).await?;
            if len == 0 {
                return Err(ProxyProtocolReadError::ClosedUnexpected);
            }

            match memchr::memchr(b'\n', &buf[0..len]) {
                Some(p) => {
                    if offset + p >= PROXY_DATA_V1_MAX_LEN {
                        return Err(ProxyProtocolReadError::InvalidDataLength(offset + p));
                    }
                    let len = p + 1;
                    let nr = stream
                        .read(&mut self.data_buf[offset..offset + len])
                        .await?;
                    assert_eq!(nr, len);
                    return Ok(offset + nr);
                }
                None => {
                    if offset + len >= PROXY_DATA_V1_MAX_LEN {
                        return Err(ProxyProtocolReadError::InvalidDataLength(offset + len));
                    }
                    let nr = stream
                        .read(&mut self.data_buf[offset..offset + len])
                        .await?;
                    assert_eq!(nr, len);
                    offset += nr;

                    if offset >= COMMON_DATA.len() {
                        if &self.data_buf[0..COMMON_DATA.len()] != COMMON_DATA {
                            return Err(ProxyProtocolReadError::InvalidMagicHeader);
                        }
                    } else if self.data_buf[0..offset] != COMMON_DATA[0..offset] {
                        return Err(ProxyProtocolReadError::InvalidMagicHeader);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::{ProxyProtocolEncoder, ProxyProtocolVersion};
    use std::str::FromStr;

    async fn run_t(client: SocketAddr, server: SocketAddr) {
        let mut encoder = ProxyProtocolEncoder::new(ProxyProtocolVersion::V1);
        let encoded = encoder.encode_tcp(client, server).unwrap();

        let reader = ProxyProtocolV1Reader::new(Duration::from_secs(1));
        let addr = reader.parse_buf(encoded).unwrap().unwrap();
        assert_eq!(addr.src_addr, client);
        assert_eq!(addr.dst_addr, server);
    }

    #[tokio::test]
    async fn t_tcp4() {
        let client = SocketAddr::from_str("192.168.0.1:56324").unwrap();
        let server = SocketAddr::from_str("192.168.0.11:443").unwrap();

        run_t(client, server).await;
    }

    #[tokio::test]
    async fn t_tcp6() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        run_t(client, server).await;
    }
}
