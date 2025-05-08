/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::io::{self, IoSlice};
use std::time::Duration;

use anyhow::anyhow;
use http::uri::PathAndQuery;
use http::{HeaderMap, Method};
use itoa::Buffer;
use log::{debug, warn};
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, BufStream};
use tokio::sync::mpsc;

use g3_http::HttpBodyDecodeReader;
use g3_http::client::HttpForwardRemoteResponse;
use g3_io_ext::LimitedWriteExt;

mod config;
pub(crate) use config::HttpExportConfig;

const BATCH_SIZE: usize = 128;

pub(crate) trait HttpExport {
    type BodyPiece;

    fn api_path(&self) -> &PathAndQuery;
    fn static_headers(&self) -> &HeaderMap;
    fn fill_body(&mut self, piece: &[Self::BodyPiece], body_buf: &mut Vec<u8>) -> usize;
    fn check_response(&self, rsp: HttpForwardRemoteResponse, body: &[u8]) -> anyhow::Result<()>;
}

pub(crate) struct HttpExportRuntime<T: HttpExport> {
    config: HttpExportConfig,
    exporter: T,
    receiver: mpsc::Receiver<T::BodyPiece>,

    recv_buf: Vec<T::BodyPiece>,
    recv_handled: usize,
    header_buf: Vec<u8>,
    fixed_header_len: usize,
    req_body_buf: Vec<u8>,
    rsp_body_buf: Vec<u8>,
    quit: bool,
    close_connection: bool,
}

impl<T: HttpExport> HttpExportRuntime<T> {
    pub(crate) fn new(
        config: HttpExportConfig,
        exporter: T,
        receiver: mpsc::Receiver<T::BodyPiece>,
    ) -> Self {
        let mut header_buf = Vec::with_capacity(1024);
        config.write_fixed_header(
            exporter.api_path(),
            &mut header_buf,
            exporter.static_headers(),
        );
        let fixed_header_len = header_buf.len();
        HttpExportRuntime {
            config,
            exporter,
            receiver,
            recv_buf: Vec::with_capacity(BATCH_SIZE),
            recv_handled: 0,
            header_buf,
            fixed_header_len,
            req_body_buf: Vec::with_capacity(2048),
            rsp_body_buf: Vec::with_capacity(256),
            quit: false,
            close_connection: false,
        }
    }

    pub(crate) async fn into_running(mut self) {
        loop {
            match self.config.connect().await {
                Ok(stream) => self.run_with_stream(BufStream::new(stream)).await,
                Err(wait) => self.drop_wait(wait).await,
            }
            if self.quit {
                break;
            }
        }
    }

    async fn drop_wait(&mut self, wait: Duration) {
        if tokio::time::timeout(wait, async {
            while self.receiver.recv().await.is_some() {
                // TODO add metrics
            }
        })
        .await
        .is_ok()
        {
            self.quit = true
        }
    }

    async fn run_with_stream<S>(&mut self, mut stream: S)
    where
        S: AsyncBufRead + AsyncWrite + Unpin,
    {
        let mut read_buf = [0u8; BATCH_SIZE];

        loop {
            if self.recv_handled < self.recv_buf.len() {
                if let Err(e) = self.send_records(&mut stream).await {
                    warn!(
                        "exporter {}: failed to send records: {e:?}",
                        self.config.exporter
                    );
                    break;
                }
                if self.close_connection {
                    break;
                }
                continue;
            } else {
                self.recv_buf.clear();
                self.recv_handled = 0;
            }

            tokio::select! {
                biased;

                r =  stream.read(&mut read_buf) => {
                    match r {
                        Ok(_) => {
                            debug!("exporter {}: connection closed by peer", self.config.exporter);
                        }
                        Err(e) => {
                            debug!("exporter {}: connection closed by peer: {e}", self.config.exporter);
                        }
                    }
                    break;
                }
                n = self.receiver.recv_many(&mut self.recv_buf, BATCH_SIZE) => {
                    if n == 0 {
                        self.quit = true;
                        break;
                    }
                }
            }
        }
    }

    async fn send_records<S>(&mut self, stream: &mut S) -> anyhow::Result<()>
    where
        S: AsyncBufRead + AsyncWrite + Unpin,
    {
        self.send_request(stream)
            .await
            .map_err(|e| anyhow!("failed to send request: {e}"))?;
        let rsp = self.recv_response(stream).await?;
        self.close_connection = !rsp.keep_alive();
        if let Err(e) = self.exporter.check_response(rsp, &self.rsp_body_buf) {
            warn!("exporter {}: error response: {e:?}", self.config.exporter);
        }
        Ok(())
    }

    async fn send_request<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.header_buf.truncate(self.fixed_header_len);
        self.req_body_buf.clear();

        let records = &self.recv_buf[self.recv_handled..];
        let handled = self.exporter.fill_body(records, &mut self.req_body_buf);
        if handled == 0 {
            warn!(
                "exporter {}: found too large piece when send request",
                self.config.exporter
            );
            // TODO add drop metrics
            self.recv_handled += 1;
        } else {
            self.recv_handled += handled;
        }

        // set content-length
        self.header_buf.extend_from_slice(b"Content-Length: ");
        let mut usize_buf = Buffer::new();
        let content_length = usize_buf.format(self.req_body_buf.len());
        self.header_buf.extend_from_slice(content_length.as_bytes());
        self.header_buf.extend_from_slice(b"\r\n\r\n");

        writer
            .write_all_vectored([
                IoSlice::new(&self.header_buf),
                IoSlice::new(&self.req_body_buf),
            ])
            .await?;

        Ok(())
    }

    async fn recv_response<R>(
        &mut self,
        reader: &mut R,
    ) -> anyhow::Result<HttpForwardRemoteResponse>
    where
        R: AsyncBufRead + Unpin,
    {
        self.rsp_body_buf.clear();

        let mut rsp = HttpForwardRemoteResponse::parse(
            reader,
            &Method::POST,
            true,
            self.config.rsp_head_max_size,
        )
        .await
        .map_err(|e| anyhow!("failed to read response header: {e}"))?;
        if let Some(body_type) = rsp.body_type(&Method::POST) {
            let mut body_reader = HttpBodyDecodeReader::new(reader, body_type, 1024);
            body_reader
                .read_to_end(&mut self.rsp_body_buf)
                .await
                .map_err(|e| anyhow!("failed to read response body: {e}"))?;
            let trailer = body_reader
                .trailer(self.config.body_line_max_len)
                .await
                .map_err(|e| anyhow!("failed to read response trailer: {e}"))?;
            if let Some(mut trailer) = trailer {
                let mut last_name = None;
                for (name, value) in trailer.drain() {
                    if let Some(n) = name {
                        last_name = Some(n);
                    };
                    if let Some(n) = &last_name {
                        rsp.append_trailer_header(n.clone(), value);
                    }
                }
            }
        }

        Ok(rsp)
    }
}
