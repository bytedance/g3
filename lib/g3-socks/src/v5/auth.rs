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

use std::collections::BTreeSet;
use std::io;

use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_types::auth::{Password, Username};
use g3_types::net::SocksAuth;

use super::{SocksAuthMethod, SocksConnectError, SocksNegotiationError, SocksRequestParseError};

pub async fn recv_methods_from_client<R>(
    clt_r: &mut R,
) -> Result<BTreeSet<SocksAuthMethod>, SocksRequestParseError>
where
    R: AsyncBufRead + Unpin,
{
    let count = clt_r.read_u8().await?;
    if count == 0 {
        return Err(SocksNegotiationError::NoAuthMethod.into());
    }
    let mut methods = BTreeSet::new();
    for _ in 0..count {
        let method = clt_r.read_u8().await?;
        let method = SocksAuthMethod::from(method);
        if let SocksAuthMethod::NoAcceptable = method {
            return Err(SocksNegotiationError::InvalidAuthMethod.into());
        }
        methods.insert(method);
    }
    Ok(methods)
}

async fn recv_method_from_remote<R>(reader: &mut R) -> Result<SocksAuthMethod, SocksConnectError>
where
    R: AsyncBufRead + Unpin,
{
    let version = reader
        .read_u8()
        .await
        .map_err(SocksConnectError::ReadFailed)?;
    if version != 0x05 {
        return Err(SocksNegotiationError::InvalidVersion.into());
    }

    let code = reader
        .read_u8()
        .await
        .map_err(SocksConnectError::ReadFailed)?;
    Ok(SocksAuthMethod::from(code))
}

pub async fn send_method_to_client<W>(clt_w: &mut W, method: &SocksAuthMethod) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = [0x05, method.code()];
    clt_w.write_all(&msg).await?;
    clt_w.flush().await
}

async fn send_methods_to_remote<W>(writer: &mut W, auth: &SocksAuth) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    if matches!(auth, SocksAuth::None) {
        let msg = [0x05, 0x01, 0x00];
        writer.write_all(&msg).await?;
    } else {
        let msg = [0x05, 0x02, auth.code(), 0x00];
        writer.write_all(&msg).await?;
    }
    writer.flush().await
}

pub(crate) async fn send_and_recv_method<R, W>(
    reader: &mut R,
    writer: &mut W,
    auth: &SocksAuth,
) -> Result<SocksAuthMethod, SocksConnectError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    send_methods_to_remote(writer, auth)
        .await
        .map_err(SocksConnectError::WriteFailed)?;
    recv_method_from_remote(reader).await
}

pub(crate) async fn proceed_with_user<R, W>(
    reader: &mut R,
    writer: &mut W,
    username: &Username,
    password: &Password,
) -> Result<(), SocksConnectError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = BytesMut::with_capacity(513);
    buf.put_u8(0x01);
    buf.put_u8(username.len());
    buf.put_slice(username.as_original().as_bytes());
    buf.put_u8(password.len());
    buf.put_slice(password.as_original().as_bytes());

    writer
        .write_all(buf.as_ref())
        .await
        .map_err(SocksConnectError::WriteFailed)?;
    writer
        .flush()
        .await
        .map_err(SocksConnectError::WriteFailed)?;

    let version = reader
        .read_u8()
        .await
        .map_err(SocksConnectError::ReadFailed)?;
    if version != 0x01 {
        return Err(SocksConnectError::UnsupportedAuthVersion);
    }

    let status = reader
        .read_u8()
        .await
        .map_err(SocksConnectError::ReadFailed)?;
    if status != 0x00 {
        Err(SocksConnectError::AuthFailed)
    } else {
        Ok(())
    }
}

pub async fn recv_user_from_client<R>(
    clt_r: &mut R,
) -> Result<(Username, Password), SocksRequestParseError>
where
    R: AsyncBufRead + Unpin,
{
    let ver = clt_r.read_u8().await?;
    if ver != 0x01 {
        return Err(SocksNegotiationError::InvalidUserAuthMsg.into());
    }

    let ul = clt_r.read_u8().await?;
    let mut buf = vec![0u8; ul as usize];
    clt_r.read_exact(&mut buf).await?;
    let username =
        std::str::from_utf8(&buf).map_err(|_| SocksNegotiationError::InvalidUserAuthMsg)?;
    let username =
        Username::from_original(username).map_err(|_| SocksNegotiationError::InvalidUserAuthMsg)?;

    let pl = clt_r.read_u8().await?;
    let mut buf = vec![0u8; pl as usize];
    clt_r.read_exact(&mut buf).await?;
    let password =
        std::str::from_utf8(&buf).map_err(|_| SocksNegotiationError::InvalidUserAuthMsg)?;
    let password =
        Password::from_original(password).map_err(|_| SocksNegotiationError::InvalidUserAuthMsg)?;

    Ok((username, password))
}

pub async fn send_user_auth_success<W>(clt_w: &mut W) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let buf = [0x01, 0x00];
    clt_w.write_all(&buf).await?;
    clt_w.flush().await
}

pub async fn send_user_auth_failure<W>(clt_w: &mut W) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let buf = [0x01, 0x01];
    clt_w.write_all(&buf).await?;
    clt_w.flush().await
}
