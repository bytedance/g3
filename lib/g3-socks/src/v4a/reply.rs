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

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{SocksNegotiationError, SocksReplyParseError};

pub enum SocksV4Reply {
    RequestGranted(SocketAddr), // the socket address is only meaningful for tcp bind
    RequestRejectedOrFailed,
    ClientIdentDNotConnected,
    UserIdNotMatch,
    Unassigned(u8),
}

impl SocksV4Reply {
    fn new(code: u8, addr: SocketAddr) -> Self {
        match code {
            90 => SocksV4Reply::RequestGranted(addr),
            91 => SocksV4Reply::RequestRejectedOrFailed,
            92 => SocksV4Reply::ClientIdentDNotConnected,
            93 => SocksV4Reply::UserIdNotMatch,
            _ => SocksV4Reply::Unassigned(code),
        }
    }

    fn code(&self) -> u8 {
        match self {
            SocksV4Reply::RequestGranted(_) => 90,
            SocksV4Reply::RequestRejectedOrFailed => 91,
            SocksV4Reply::ClientIdentDNotConnected => 92,
            SocksV4Reply::UserIdNotMatch => 93,
            SocksV4Reply::Unassigned(code) => *code,
        }
    }

    pub(crate) const fn error_message(&self) -> &'static str {
        match self {
            SocksV4Reply::RequestGranted(_) => "request granted",
            SocksV4Reply::RequestRejectedOrFailed => "request rejected or failed",
            SocksV4Reply::ClientIdentDNotConnected => {
                "request rejected becasue SOCKS server cannot connect to identd on the client"
            }
            SocksV4Reply::UserIdNotMatch => {
                "request rejected because the client program and identd report different user-ids"
            }
            SocksV4Reply::Unassigned(_) => "unassigned reply code",
        }
    }

    pub(crate) async fn recv<R>(reader: &mut R) -> Result<Self, SocksReplyParseError>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf).await?;

        let version = buf[0];
        if version != 0x00 {
            return Err(SocksNegotiationError::InvalidVersion.into());
        }

        let code = buf[1];

        let ip_bytes: [u8; 4] = buf[4..8].try_into().unwrap();

        let port = ((buf[3] as u16) << 8) + (buf[4] as u16);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port);

        Ok(SocksV4Reply::new(code, addr))
    }

    pub async fn send<W>(&self, clt_w: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let buf: [u8; 8] = [0, self.code(), 0, 0, 0, 0, 0, 0];
        clt_w.write_all(&buf).await?;
        clt_w.flush().await?;
        Ok(())
    }

    pub fn request_granted() -> Self {
        SocksV4Reply::RequestGranted(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
    }
}
