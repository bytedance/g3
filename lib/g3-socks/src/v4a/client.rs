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
