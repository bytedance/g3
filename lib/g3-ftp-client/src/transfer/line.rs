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

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite, BufStream};

use g3_io_ext::LimitedBufReadExt;

use crate::config::FtpTransferConfig;
use crate::error::FtpLineDataReadError;

#[async_trait]
pub trait FtpLineDataReceiver {
    async fn recv_line(&mut self, line: &str);
    fn should_return_early(&self) -> bool;
}

pub(crate) struct FtpLineDataTransfer<T: AsyncRead + AsyncWrite> {
    io: BufStream<T>,
    read_lines: usize,
    max_lines: usize,
    line_buf: Vec<u8>,
}

impl<T> FtpLineDataTransfer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(io: T, config: &FtpTransferConfig) -> Self {
        FtpLineDataTransfer {
            io: BufStream::new(io),
            read_lines: 0,
            max_lines: config.list_max_entries,
            line_buf: Vec::with_capacity(config.list_max_line_len),
        }
    }

    async fn send_buf_to_receiver<R>(
        &mut self,
        receiver: &mut R,
    ) -> Result<(), FtpLineDataReadError>
    where
        R: FtpLineDataReceiver,
    {
        let s = std::str::from_utf8(&self.line_buf)
            .map_err(|_| FtpLineDataReadError::UnsupportedEncoding)?;
        receiver.recv_line(s).await;
        if receiver.should_return_early() {
            self.read_lines += 1;
            return Err(FtpLineDataReadError::AbortedByCallback);
        }
        self.line_buf.clear();
        Ok(())
    }

    pub(crate) async fn read_to_end<R>(
        mut self,
        receiver: &mut R,
    ) -> Result<(), FtpLineDataReadError>
    where
        R: FtpLineDataReceiver,
    {
        if !self.line_buf.is_empty() {
            self.send_buf_to_receiver(receiver).await?;
        }

        for i in self.read_lines..self.max_lines {
            let (found, nr) = self
                .io
                .limited_read_until(b'\n', self.line_buf.capacity(), &mut self.line_buf)
                .await?;
            if nr == 0 {
                return Ok(());
            }

            if !found {
                return Err(FtpLineDataReadError::LineTooLong(i + 1));
            }

            self.send_buf_to_receiver(receiver).await?;
        }

        Err(FtpLineDataReadError::TooManyLines)
    }
}
