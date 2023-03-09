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

use std::convert::TryInto;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{SocksNegotiationError, SocksReplyParseError};

pub enum Socks5Reply {
    Succeeded(SocketAddr),
    GeneralServerFailure,
    ForbiddenByRule,
    NetworkUnreachable,
    HostUnreachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddressTypeNotSupported,
    ConnectionTimedOut,
    Unassigned(u8),
}

impl Socks5Reply {
    fn new(code: u8, addr: SocketAddr) -> Self {
        match code {
            0x00 => Socks5Reply::Succeeded(addr),
            0x01 => Socks5Reply::GeneralServerFailure,
            0x02 => Socks5Reply::ForbiddenByRule,
            0x03 => Socks5Reply::NetworkUnreachable,
            0x04 => Socks5Reply::HostUnreachable,
            0x05 => Socks5Reply::ConnectionRefused,
            0x06 => Socks5Reply::TtlExpired,
            0x07 => Socks5Reply::CommandNotSupported,
            0x08 => Socks5Reply::AddressTypeNotSupported,
            0x09 => Socks5Reply::ConnectionTimedOut,
            n => Socks5Reply::Unassigned(n),
        }
    }

    fn code(&self) -> u8 {
        match self {
            Socks5Reply::Succeeded(_) => 0x00,
            Socks5Reply::GeneralServerFailure => 0x01,
            Socks5Reply::ForbiddenByRule => 0x02,
            Socks5Reply::NetworkUnreachable => 0x03,
            Socks5Reply::HostUnreachable => 0x04,
            Socks5Reply::ConnectionRefused => 0x05,
            Socks5Reply::TtlExpired => 0x06,
            Socks5Reply::CommandNotSupported => 0x07,
            Socks5Reply::AddressTypeNotSupported => 0x08,
            Socks5Reply::ConnectionTimedOut => 0x09,
            Socks5Reply::Unassigned(n) => *n,
        }
    }

    pub(crate) const fn error_message(&self) -> &'static str {
        match self {
            // message from rfc1928
            Socks5Reply::Succeeded(_) => "Succeeded",
            Socks5Reply::GeneralServerFailure => "General SOCKS server failure",
            Socks5Reply::ForbiddenByRule => "Connection not allowed by ruleset",
            Socks5Reply::NetworkUnreachable => "Network unreachable",
            Socks5Reply::HostUnreachable => "Host unreachable",
            Socks5Reply::ConnectionRefused => "Connection refused",
            Socks5Reply::TtlExpired => "TTL expired",
            Socks5Reply::CommandNotSupported => "Command not supported",
            Socks5Reply::AddressTypeNotSupported => "Address type not supported",
            // message from socks-6-09
            Socks5Reply::ConnectionTimedOut => "Connection attempt timed out",
            Socks5Reply::Unassigned(_) => "unassigned reply code",
        }
    }

    pub(crate) async fn recv<R>(reader: &mut R) -> Result<Self, SocksReplyParseError>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).await?;
        let version = buf[0];
        if version != 0x05 {
            return Err(SocksNegotiationError::InvalidVersion.into());
        }

        let code = buf[1];

        let _rsv = buf[2];

        let addr = match buf[3] {
            0x01 => {
                let mut left_bytes = [0u8; 6];
                reader.read_exact(&mut left_bytes).await?;
                let ip_bytes: [u8; 4] = left_bytes[0..4].try_into().unwrap();
                let port_bytes: [u8; 2] = left_bytes[4..6].try_into().unwrap();
                let port = u16::from_be_bytes(port_bytes);
                SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port)
            }
            0x03 => return Err(SocksNegotiationError::InvalidAddrType.into()),
            0x04 => {
                let mut left_bytes: [u8; 18] = [0; 18];
                reader.read_exact(&mut left_bytes).await?;
                let ip_bytes: [u8; 16] = left_bytes[0..16].try_into().unwrap();
                let port_bytes: [u8; 2] = left_bytes[16..18].try_into().unwrap();
                let port = u16::from_be_bytes(port_bytes);
                SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ip_bytes)), port)
            }
            _ => return Err(SocksNegotiationError::InvalidAddrType.into()),
        };

        Ok(Socks5Reply::new(code, addr))
    }

    pub async fn send<W>(&self, clt_w: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u8(0x05);
        buf.put_u8(self.code());
        buf.put_u8(0x00);
        match self {
            Socks5Reply::Succeeded(addr) => match addr {
                SocketAddr::V4(addr4) => {
                    buf.put_u8(0x01);
                    buf.put_slice(&addr4.ip().octets());
                    buf.put_u16(addr4.port());
                }
                SocketAddr::V6(addr6) => {
                    let ip6 = addr6.ip();
                    let port = addr6.port();
                    match ip6.to_ipv4_mapped() {
                        Some(ip4) => {
                            buf.put_u8(0x01);
                            buf.put_slice(&ip4.octets());
                            buf.put_u16(port);
                        }
                        None => {
                            buf.put_u8(0x04);
                            buf.put_slice(&ip6.octets());
                            buf.put_u16(port);
                        }
                    }
                }
            },
            _ => {
                buf.put_u8(0x01);
                buf.put_slice(&[0x00, 0x00, 0x00, 0x00]);
                buf.put_slice(&[0x00, 0x00]);
            }
        }
        clt_w.write_all(buf.as_ref()).await?;
        clt_w.flush().await
    }
}
