/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncBufRead, AsyncWrite};

use g3_types::net::{HttpAuth, UpstreamAddr};

use super::{HttpConnectError, HttpConnectRequest, HttpConnectResponse};

pub async fn http_connect_to<S>(
    buf_stream: &mut S,
    auth: &HttpAuth,
    addr: &UpstreamAddr,
) -> Result<(), HttpConnectError>
where
    S: AsyncBufRead + AsyncWrite + Unpin,
{
    let mut req = HttpConnectRequest::new(addr, &[]);

    match auth {
        HttpAuth::None => {}
        HttpAuth::Basic(a) => {
            let line = crate::header::proxy_authorization_basic(&a.username, &a.password);
            req.append_dyn_header(line);
        }
    }

    req.send(buf_stream)
        .await
        .map_err(HttpConnectError::WriteFailed)?;

    let _ = HttpConnectResponse::recv(buf_stream, 2048).await?;

    Ok(())
}
