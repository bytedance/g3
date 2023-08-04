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

use std::sync::Arc;

use anyhow::anyhow;
use http::{Method, StatusCode};
use serde_json::{Map, Value};
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::TcpStream;

use g3_http::client::HttpForwardRemoteResponse;
use g3_http::HttpBodyReader;
use g3_io_ext::LimitedBufReadExt;

use super::RegisterConfig;

pub struct RegisterTask {
    config: Arc<RegisterConfig>,
    stream: BufStream<TcpStream>,
}

impl RegisterTask {
    pub async fn new(config: Arc<RegisterConfig>) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(config.upstream.to_string())
            .await
            .map_err(|e| anyhow!("failed to connect to {}: {e:?}", config.upstream))?;
        Ok(RegisterTask {
            config,
            stream: BufStream::new(stream),
        })
    }

    pub async fn reopen(&mut self) -> anyhow::Result<()> {
        let stream = TcpStream::connect(self.config.upstream.to_string())
            .await
            .map_err(|e| anyhow!("failed to connect to {}: {e:?}", self.config.upstream))?;
        self.stream = BufStream::new(stream);
        Ok(())
    }

    pub async fn register(&mut self, mut content: Map<String, Value>) -> anyhow::Result<()> {
        for (k, v) in &self.config.extra_data {
            content.insert(k.to_string(), Value::String(v.to_string()));
        }

        let body = Value::Object(content).to_string();
        let data = format!(
            "POST {} HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: Keep-Alive\r\n\
             \r\n{body}",
            self.config.register_path,
            body.len()
        );

        self.write_request(data.as_bytes()).await?;
        self.check_response(Method::POST).await
    }

    pub async fn ping_until_end(&mut self) -> anyhow::Result<()> {
        let data = format!(
            "GET {} HTTP/1.1\r\n\
             Content-Length: 0\r\n\
             Connection: Keep-Alive\r\n
             \r\n",
            self.config.ping_path
        );

        let mut interval = tokio::time::interval(self.config.ping_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.write_request(data.as_bytes()).await?;
                    self.check_response(Method::GET).await?;
                }
                _ = self.stream.fill_wait_data() => {
                    return Err(anyhow!("upstream closed connection"));
                }
            }
        }
    }

    async fn write_request(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.stream
            .write_all(data)
            .await
            .map_err(|e| anyhow!("failed to write data: {e:?}"))?;
        self.stream
            .flush()
            .await
            .map_err(|e| anyhow!("failed to write data: {e:?}"))
    }

    async fn check_response(&mut self, method: Method) -> anyhow::Result<()> {
        let rsp = HttpForwardRemoteResponse::parse(&mut self.stream, &method, true, 4096)
            .await
            .map_err(|e| anyhow!("failed to recv response: {e}"))?;
        if rsp.code != StatusCode::OK {
            return Err(anyhow!("unexpected response: {} {}", rsp.code, rsp.reason));
        }

        // recv body
        if let Some(body_type) = rsp.body_type(&method) {
            let mut body_reader = HttpBodyReader::new(&mut self.stream, body_type, 1024);
            let mut sink = tokio::io::sink();
            tokio::io::copy(&mut body_reader, &mut sink)
                .await
                .map_err(|e| anyhow!("failed to read response body: {e:?}"))?;
        }

        Ok(())
    }
}
