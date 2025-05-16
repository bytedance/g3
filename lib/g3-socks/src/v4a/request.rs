/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, Ipv4Addr};

use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite};

use g3_io_ext::{LimitedBufReadExt, LimitedWriteExt};
use g3_types::net::{Host, UpstreamAddr};

use super::{SocksCommand, SocksNegotiationError, SocksRequestParseError};

pub struct SocksV4aRequest {
    pub command: SocksCommand,
    pub upstream: UpstreamAddr,
    #[allow(unused)]
    pub user_id: String,
}

impl SocksV4aRequest {
    /// parse the first packet for Socks V4 & V4a
    /// the version code should has already been read and checked
    pub async fn recv<R>(clt_r: &mut R) -> Result<Self, SocksRequestParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let command = SocksCommand::try_from(clt_r.read_u8().await?)?;

        if matches!(command, SocksCommand::UdpAssociate) {
            return Err(SocksNegotiationError::InvalidCommand.into());
        }

        let port = clt_r.read_u16().await?;
        let mut ip_bytes: [u8; 4] = [0; 4];
        clt_r.read_exact(&mut ip_bytes).await?;

        const USER_ID_MAX_LEN: usize = 512;
        let mut user_id_buf: Vec<u8> = Vec::with_capacity(USER_ID_MAX_LEN);
        let (found, nr) = clt_r
            .limited_read_until(0x0, USER_ID_MAX_LEN + 1, &mut user_id_buf)
            .await?;
        if nr == 0 {
            return Err(SocksRequestParseError::ClientClosed);
        }
        if !found || nr > USER_ID_MAX_LEN {
            return Err(SocksNegotiationError::InvalidUserIdString.into());
        }
        user_id_buf.truncate(nr - 1);
        let user_id = if user_id_buf.is_empty() {
            String::new()
        } else {
            String::from_utf8(user_id_buf)
                .map_err(|_| SocksNegotiationError::InvalidUserIdString)?
        };

        let upstream = match ip_bytes {
            [0, 0, 0, 0] => UpstreamAddr::from_ip_and_port(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
            [0, 0, 0, _] => {
                const DOMAIN_MAX_LEN: usize = 256;
                let mut domain: Vec<u8> = Vec::with_capacity(DOMAIN_MAX_LEN);
                let (found, nr) = clt_r
                    .limited_read_until(0x0, DOMAIN_MAX_LEN + 1, &mut domain)
                    .await?;
                match nr {
                    0 => return Err(SocksRequestParseError::ClientClosed),
                    1 => return Err(SocksNegotiationError::InvalidDomainString.into()),
                    _ => {}
                }
                if !found || nr > DOMAIN_MAX_LEN {
                    return Err(SocksNegotiationError::InvalidDomainString.into());
                }
                domain.truncate(nr - 1);
                let domain = std::str::from_utf8(&domain)
                    .map_err(|_| SocksNegotiationError::InvalidDomainString)?;
                UpstreamAddr::from_host_str_and_port(domain, port)
                    .map_err(|_| SocksNegotiationError::InvalidDomainString)?
            }
            _ => UpstreamAddr::from_ip_and_port(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port),
        };

        Ok(Self {
            command,
            upstream,
            user_id,
        })
    }

    pub(crate) async fn send<W>(
        writer: &mut W,
        command: SocksCommand,
        addr: &UpstreamAddr,
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf_len = 1 + 1 + 2 + 4 + 1;
        let buf = match addr.host() {
            Host::Ip(IpAddr::V4(ip4)) => {
                let mut buf = BytesMut::with_capacity(buf_len);
                buf.put_u8(0x04);
                buf.put_u8(command.code());
                buf.put_u16(addr.port());
                buf.put_slice(&ip4.octets());
                // we have no support for userid
                buf.put_u8(0x00);
                buf
            }
            Host::Ip(IpAddr::V6(_ip6)) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "ipv6 remote address is not supported",
                ));
            }
            Host::Domain(domain) => {
                buf_len += domain.len() + 1;
                let mut buf = BytesMut::with_capacity(buf_len);
                buf.put_u8(0x04);
                buf.put_u8(command.code());
                buf.put_u16(addr.port());
                buf.put_slice(&[0x00, 0x00, 0x00, 0x01]);
                // we have no support for userid
                buf.put_u8(0x00);
                buf.put_slice(domain.as_bytes());
                buf.put_u8(0x00);
                buf
            }
        };
        writer.write_all_flush(buf.as_ref()).await
    }
}
