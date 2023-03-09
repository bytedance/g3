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

use tokio::io::{AsyncBufRead, AsyncWrite};

use g3_types::net::{HttpAuth, UpstreamAddr};

use super::{HttpConnectError, HttpConnectRequest, HttpConnectResponse};

pub async fn http_connect_to<R, W>(
    reader: &mut R,
    writer: &mut W,
    auth: &HttpAuth,
    addr: &UpstreamAddr,
) -> Result<(), HttpConnectError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut req = HttpConnectRequest::new(addr, &[]);

    match auth {
        HttpAuth::None => {}
        HttpAuth::Basic(a) => {
            let line = crate::header::proxy_authorization_basic(&a.username, &a.password);
            req.append_dyn_header(line);
        }
    }

    req.send(writer)
        .await
        .map_err(HttpConnectError::WriteFailed)?;

    let _ = HttpConnectResponse::recv(reader, 2048).await?;

    Ok(())
}
