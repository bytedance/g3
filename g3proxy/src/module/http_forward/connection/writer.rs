/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, IoSlice};

use bytes::BufMut;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_io_ext::LimitedWriteExt;
use g3_types::net::UpstreamAddr;

use super::HttpProxyClientRequest;
use crate::module::http_header;

pub(crate) async fn send_req_header_via_proxy<W>(
    writer: &mut W,
    req: &HttpProxyClientRequest,
    body: Option<&[u8]>,
    upstream: &UpstreamAddr,
    append_header_lines: &[String],
    pass_userid: Option<&str>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    const RESERVED_LEN_FOR_EXTRA_HEADERS: usize = 256;
    let mut buf = req.partial_serialize_for_proxy(upstream, RESERVED_LEN_FOR_EXTRA_HEADERS);
    for line in append_header_lines {
        buf.put_slice(line.as_bytes());
    }
    if let Some(userid) = pass_userid {
        let header = http_header::proxy_authorization_basic_pass(userid);
        buf.put_slice(header.as_bytes());
    }
    buf.put_slice(b"\r\n");

    send_request_header(writer, buf.as_slice(), body).await
}

pub(crate) async fn send_req_header_to_origin<W>(
    writer: &mut W,
    req: &HttpProxyClientRequest,
    body: Option<&[u8]>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let buf = req.serialize_for_origin();
    send_request_header(writer, buf.as_slice(), body).await
}

async fn send_request_header<W>(
    writer: &mut W,
    header: &[u8],
    body: Option<&[u8]>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    if let Some(body) = body {
        writer
            .write_all_vectored([IoSlice::new(header), IoSlice::new(body)])
            .await?;
        Ok(())
    } else {
        writer.write_all(header).await
    }
}
