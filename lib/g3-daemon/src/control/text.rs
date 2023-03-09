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

use std::str::SplitWhitespace;

use anyhow::anyhow;
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Duration;
use yaml_rust::Yaml;

use g3_io_ext::LimitedBufReadExt;

use super::{CtlProtoType, GeneralControllerConfig};

const TEXT_COMMAND_MAX_LEN: usize = 1024;

pub(super) struct TextCtlCtx<'a, R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    reader: &'a mut R,
    writer: &'a mut W,
    config: &'a mut GeneralControllerConfig,
    buf: Vec<u8>,
}

impl<'a, R, W> TextCtlCtx<'a, R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub(super) fn new(
        reader: &'a mut R,
        writer: &'a mut W,
        config: &'a mut GeneralControllerConfig,
    ) -> Self {
        TextCtlCtx {
            reader,
            writer,
            config,
            buf: Vec::with_capacity(TEXT_COMMAND_MAX_LEN),
        }
    }

    pub(super) async fn run(&mut self) -> anyhow::Result<CtlProtoType> {
        loop {
            let (command, end) = self.read().await?;
            if end {
                return Ok(CtlProtoType::End);
            }
            if command.is_empty() {
                continue;
            }

            let (mut response, ctl_type) = self.handle(&command).await;

            if !response.is_empty() {
                response.push('\n');
                self.send_response(&response).await?;
            }

            if ctl_type != CtlProtoType::Text {
                return Ok(ctl_type);
            }
        }
    }

    async fn read(&mut self) -> anyhow::Result<(String, bool)> {
        self.buf.clear();
        match tokio::time::timeout(
            Duration::from_secs(self.config.recv_timeout),
            self.reader
                .limited_read_until(b'\n', TEXT_COMMAND_MAX_LEN, &mut self.buf),
        )
        .await?
        {
            Ok((_, 0)) => {
                // Client disconnected
                Ok((String::new(), true))
            }
            Ok((false, n)) => {
                if n < TEXT_COMMAND_MAX_LEN {
                    // Client disconnected
                    Ok((String::new(), true))
                } else {
                    Err(anyhow!("too long command"))
                }
            }
            Ok((true, n)) => {
                let command = std::str::from_utf8(&self.buf[0..n])
                    .map_err(|e| anyhow!("invalid utf-8 string: {e}"))?;
                Ok((command.trim_end().to_string(), false))
            }
            Err(e) => Err(anyhow!("read: {e}")),
        }
    }

    async fn send_response(&mut self, response: &str) -> anyhow::Result<()> {
        let send_timeout = self.config.send_timeout;
        let fut = async {
            self.writer.write_all(response.as_bytes()).await?;
            self.writer.flush().await
        };
        match tokio::time::timeout(Duration::from_secs(send_timeout), fut).await? {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("write: {e}")),
        }
    }

    async fn handle(&mut self, command: &str) -> (String, CtlProtoType) {
        let command = command.to_lowercase();
        let mut iter = command.split_whitespace();
        let cmd = iter.next();
        let mut ctl_type = CtlProtoType::Text;
        let response = match cmd {
            Some("quit") => {
                ctl_type = CtlProtoType::End;
                Ok(String::new())
            }
            Some("capnp") => {
                ctl_type = CtlProtoType::CapnP;
                Ok(String::new())
            }
            Some("set") => self.set(iter),
            Some(k) => Err(anyhow!("unknown command {k}")),
            None => Ok(String::new()),
        };
        match response {
            Ok(response) => (response, ctl_type),
            Err(e) => (format!("Error: {e}"), ctl_type),
        }
    }

    fn set(&mut self, mut iter: SplitWhitespace) -> anyhow::Result<String> {
        if let Some(key) = iter.next() {
            if let Some(value) = iter.next() {
                self.config.set(key, &Yaml::String(value.to_string()))?;
                Ok(format!("{key} = {value}"))
            } else {
                Err(anyhow!("no value for {key} found"))
            }
        } else {
            Err(anyhow!("no key to set"))
        }
    }
}
