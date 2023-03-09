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

use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite, BufReader};

use g3_types::net::{SocksAuth, UpstreamAddr};

use super::{auth, Socks5Reply, Socks5Request, SocksAuthMethod, SocksCommand, SocksConnectError};

async fn socks5_login<R, W>(
    reader: &mut R,
    writer: &mut W,
    auth: &SocksAuth,
) -> Result<(), SocksConnectError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(reader);
    let auth_method = auth::send_and_recv_method(&mut reader, writer, auth).await?;
    match auth_method {
        SocksAuthMethod::None => {}
        SocksAuthMethod::User => {
            if let SocksAuth::User(username, password) = auth {
                auth::proceed_with_user(&mut reader, writer, username, password).await?;
            } else {
                return Err(SocksConnectError::NoAuthMethodAvailable);
            }
        }
        _ => return Err(SocksConnectError::NoAuthMethodAvailable),
    }

    // the buf reader is dropped
    Ok(())
}

/// tcp connect to a socks5 proxy
///
/// return the local bind address at the server side
pub async fn socks5_connect_to<R, W>(
    reader: &mut R,
    writer: &mut W,
    auth: &SocksAuth,
    addr: &UpstreamAddr,
) -> Result<SocketAddr, SocksConnectError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    socks5_login(reader, writer, auth).await?;

    Socks5Request::send(writer, SocksCommand::TcpConnect, addr)
        .await
        .map_err(SocksConnectError::WriteFailed)?;

    let rsp = Socks5Reply::recv(reader).await?;
    match rsp {
        Socks5Reply::Succeeded(addr) => Ok(addr),
        Socks5Reply::ConnectionTimedOut => Err(SocksConnectError::PeerTimeout),
        _ => Err(SocksConnectError::RequestFailed(format!(
            "request failed: {}",
            rsp.error_message()
        ))),
    }
}

/// udp associate to a socks5 proxy
///
/// return the socket address that the client should send packets to
pub async fn socks5_udp_associate<R, W>(
    reader: &mut R,
    writer: &mut W,
    auth: &SocksAuth,
    local_udp_addr: SocketAddr,
) -> Result<SocketAddr, SocksConnectError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    socks5_login(reader, writer, auth).await?;

    let addr = UpstreamAddr::from_ip_and_port(local_udp_addr.ip(), local_udp_addr.port());
    Socks5Request::send(writer, SocksCommand::UdpAssociate, &addr)
        .await
        .map_err(SocksConnectError::WriteFailed)?;

    let rsp = Socks5Reply::recv(reader).await?;
    match rsp {
        Socks5Reply::Succeeded(addr) => Ok(addr),
        Socks5Reply::ConnectionTimedOut => Err(SocksConnectError::PeerTimeout),
        _ => Err(SocksConnectError::RequestFailed(format!(
            "request failed: {}",
            rsp.error_message()
        ))),
    }
}
