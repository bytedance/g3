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

use std::convert::TryFrom;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_types::net::{Host, UpstreamAddr};

use super::{SocksCommand, SocksNegotiationError, SocksRequestParseError};

pub struct Socks5Request {
    pub command: SocksCommand,
    pub upstream: UpstreamAddr,
}

impl Socks5Request {
    pub async fn recv<R>(clt_r: &mut R) -> Result<Self, SocksRequestParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let version = clt_r.read_u8().await?;
        if version != 0x05 {
            return Err(SocksNegotiationError::InvalidVersion.into());
        }

        let command = SocksCommand::try_from(clt_r.read_u8().await?)?;

        let _rsv = clt_r.read_u8().await?;

        let upstream = match clt_r.read_u8().await? {
            0x01 => {
                let mut ip_bytes: [u8; 4] = [0; 4];
                clt_r.read_exact(&mut ip_bytes).await?;
                let port = clt_r.read_u16().await?;
                UpstreamAddr::from_ip_and_port(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port)
            }
            0x03 => {
                let len = clt_r.read_u8().await?;
                if len == 0 {
                    return Err(SocksNegotiationError::InvalidDomainString.into());
                }
                let mut domain = vec![0u8; len as usize];
                clt_r.read_exact(&mut domain).await?;
                let domain = std::str::from_utf8(&domain)
                    .map_err(|_| SocksNegotiationError::InvalidDomainString)?;
                let port = clt_r.read_u16().await?;
                UpstreamAddr::from_host_str_and_port(domain, port)
                    .map_err(|_| SocksNegotiationError::InvalidDomainString)?
            }
            0x04 => {
                let mut ip_bytes: [u8; 16] = [0; 16];
                clt_r.read_exact(&mut ip_bytes).await?;
                let port = clt_r.read_u16().await?;
                UpstreamAddr::from_ip_and_port(IpAddr::V6(Ipv6Addr::from(ip_bytes)), port)
            }
            _ => return Err(SocksNegotiationError::InvalidAddrType.into()),
        };

        Ok(Self { command, upstream })
    }

    pub fn udp_peer_addr(&self) -> Result<Option<SocketAddr>, SocksRequestParseError> {
        match self.upstream.host() {
            Host::Ip(ip) => Ok(Some(SocketAddr::new(*ip, self.upstream.port()))),
            Host::Domain(domain) => {
                if domain.eq("0") {
                    // to be compatible with pysocks
                    Ok(None)
                } else {
                    Err(SocksRequestParseError::InvalidUdpPeerAddress)
                }
            }
        }
    }

    pub(crate) async fn send<W>(
        writer: &mut W,
        command: SocksCommand,
        addr: &UpstreamAddr,
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u8(0x05);
        buf.put_u8(command.code());
        buf.put_u8(0x00);
        match addr.host() {
            Host::Domain(domain) => {
                let len: u8 = domain.len() as u8;
                buf.put_u8(0x03);
                buf.put_u8(len);
                buf.put_slice(&domain.as_bytes()[0..len as usize]);
                buf.put_u16(addr.port());
            }
            Host::Ip(IpAddr::V4(ip4)) => {
                buf.put_u8(0x01);
                buf.put_slice(&ip4.octets());
                buf.put_u16(addr.port());
            }
            Host::Ip(IpAddr::V6(ip6)) => {
                // No need to do ipv4 mapped address check here
                buf.put_u8(0x04);
                buf.put_slice(&ip6.octets());
                buf.put_u16(addr.port());
            }
        }
        writer.write_all(buf.as_ref()).await?;
        writer.flush().await
    }
}
