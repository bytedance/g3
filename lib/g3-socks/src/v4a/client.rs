/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite};

use g3_types::net::UpstreamAddr;

use super::{SocksConnectError, SocksV4Reply, SocksV4aRequest};
use crate::SocksCommand;

pub async fn socks4a_connect_to<S>(
    stream: &mut S,
    addr: &UpstreamAddr,
) -> Result<(), SocksConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    SocksV4aRequest::send(stream, SocksCommand::TcpConnect, addr)
        .await
        .map_err(SocksConnectError::WriteFailed)?;

    let rsp = SocksV4Reply::recv(stream).await?;
    match rsp {
        SocksV4Reply::RequestGranted(_) => Ok(()),
        _ => Err(SocksConnectError::RequestFailed(format!(
            "request failed: {}",
            rsp.error_message()
        ))),
    }
}
