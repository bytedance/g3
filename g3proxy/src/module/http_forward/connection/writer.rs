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

use bytes::BufMut;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_types::net::UpstreamAddr;

use super::HttpProxyClientRequest;
use crate::module::http_header;

pub(crate) async fn send_req_header_via_proxy<W>(
    writer: &mut W,
    req: &HttpProxyClientRequest,
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

    writer.write_all(buf.as_ref()).await
}

pub(crate) async fn send_req_header_to_origin<W>(
    writer: &mut W,
    req: &HttpProxyClientRequest,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let buf = req.serialize_for_origin();
    writer.write_all(buf.as_ref()).await
}
