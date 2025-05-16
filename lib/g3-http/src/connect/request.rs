/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use g3_types::net::UpstreamAddr;

/// the extra header lines should end with \r\n
pub struct HttpConnectRequest<'a> {
    host: &'a UpstreamAddr,
    static_headers: &'a [String],
    dyn_headers: Vec<String>,
}

impl<'a> HttpConnectRequest<'a> {
    pub fn new(host: &'a UpstreamAddr, static_headers: &'a [String]) -> Self {
        HttpConnectRequest {
            host,
            static_headers,
            dyn_headers: Vec::new(),
        }
    }

    pub fn append_dyn_header(&mut self, line: String) {
        debug_assert!(line.ends_with("\r\n"));
        self.dyn_headers.push(line);
    }

    /// the extra header lines should end with \r\n
    pub async fn send<W>(&'a self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf_writer = BufWriter::new(writer);
        buf_writer
            .write_all(format!("CONNECT {} HTTP/1.1\r\n", self.host).as_bytes())
            .await?;
        buf_writer
            .write_all(format!("Host: {}\r\n", self.host).as_bytes())
            .await?;
        buf_writer.write_all(b"Connection: keep-alive\r\n").await?;
        for line in self.static_headers {
            debug_assert!(line.ends_with("\r\n"));
            buf_writer.write_all(line.as_bytes()).await?;
        }
        for line in &self.dyn_headers {
            buf_writer.write_all(line.as_bytes()).await?;
        }
        buf_writer.write_all(b"\r\n").await?;
        buf_writer.flush().await
    }
}
