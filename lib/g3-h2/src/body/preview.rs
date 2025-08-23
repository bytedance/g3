/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::io;
use std::io::IoSlice;
use std::task::Poll;
use std::time::Duration;

use bytes::Bytes;
use h2::{RecvStream, SendStream};
use thiserror::Error;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_io_ext::{IdleCheck, IdleForceQuitReason, LimitedWriteExt};

#[derive(Debug, Error)]
pub enum H2PreviewError {
    #[error("read error: {0}")]
    ReadDataFailed(h2::Error),
    #[error("read idle")]
    ReadIdle,
    #[error("force quit: {0:?}")]
    IdleForceQuit(IdleForceQuitReason),
}

pub struct H2PreviewData {
    max_size: usize,
    received: usize,
    buffer: Vec<u8>,
    left: Option<Bytes>,
    end_of_data: bool,
}

impl H2PreviewData {
    pub fn new(preview_size: usize) -> Self {
        H2PreviewData {
            max_size: preview_size,
            received: 0,
            buffer: Vec::with_capacity(preview_size),
            left: None,
            end_of_data: false,
        }
    }

    #[inline]
    pub fn end_of_data(&self) -> bool {
        self.end_of_data
    }

    #[inline]
    pub fn preview_size(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn take_left(&mut self) -> Option<Bytes> {
        self.left.take()
    }

    pub async fn recv_all<I: IdleCheck>(
        &mut self,
        clt_body: &mut RecvStream,
        idle_checker: &I,
    ) -> Result<(), H2PreviewError> {
        let mut is_active = false;
        let mut idle_count = 0;

        let mut idle_interval = idle_checker.interval_timer();

        loop {
            tokio::select! {
                biased;

                r = clt_body.data() => {
                    let mut data = match r {
                        Some(Ok(data)) => data,
                        Some(Err(e)) => {
                            return Err(H2PreviewError::ReadDataFailed(e));
                        }
                        None => break,
                    };
                    if data.is_empty() {
                        continue;
                    }

                    self.received += data.len();
                    match self.received.checked_sub(self.max_size) {
                        Some(0) => {
                            self.buffer.extend_from_slice(&data);
                            return Ok(());
                        }
                        Some(left) => {
                            let keep = data.len() - left;
                            let left = data.split_off(keep);
                            self.buffer.extend_from_slice(&data);
                            self.left = Some(left);
                            return Ok(());
                        }
                        None => self.buffer.extend_from_slice(&data),
                    }
                }
                n = idle_interval.tick() => {
                    if !is_active {
                        idle_count += n;

                        let quit = idle_checker.check_quit(idle_count);
                        if quit {
                            return Err(H2PreviewError::ReadIdle);
                        }
                    } else {
                        idle_count = 0;
                        is_active = false;
                    }

                    if let Some(reason) = idle_checker.check_force_quit() {
                        return Err(H2PreviewError::IdleForceQuit(reason));
                    }
                }
            }
        }

        self.end_of_data = true;
        Ok(())
    }

    pub async fn recv_initial(
        &mut self,
        clt_body: &mut RecvStream,
        timeout: Duration,
    ) -> Result<(), H2PreviewError> {
        match tokio::time::timeout(timeout, clt_body.data()).await {
            Ok(Some(Ok(mut data))) => {
                if !data.is_empty() {
                    self.received += data.len();
                    match data.len().checked_sub(self.max_size) {
                        Some(0) => {
                            self.buffer.extend_from_slice(&data);
                            return Ok(());
                        }
                        Some(left) => {
                            let keep = data.len() - left;
                            let left = data.split_off(keep);
                            self.buffer.extend_from_slice(&data);
                            self.left = Some(left);
                            return Ok(());
                        }
                        None => self.buffer.extend_from_slice(&data),
                    }
                }
            }
            Ok(Some(Err(e))) => return Err(H2PreviewError::ReadDataFailed(e)),
            Ok(None) => {
                self.end_of_data = true;
                return Ok(());
            }
            Err(_) => return Ok(()),
        }

        loop {
            match poll_fn(|cx| match clt_body.poll_data(cx) {
                Poll::Ready(Some(Ok(data))) => Poll::Ready(Some(Ok(data))),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => {
                    self.end_of_data = true;
                    Poll::Ready(None)
                }
                Poll::Pending => Poll::Ready(None),
            })
            .await
            {
                Some(Ok(mut data)) => {
                    if data.is_empty() {
                        continue;
                    }

                    self.received += data.len();
                    match self.received.checked_sub(self.max_size) {
                        Some(0) => {
                            self.buffer.extend_from_slice(&data);
                            return Ok(());
                        }
                        Some(left) => {
                            let keep = data.len() - left;
                            let left = data.split_off(keep);
                            self.buffer.extend_from_slice(&data);
                            self.left = Some(left);
                            return Ok(());
                        }
                        None => self.buffer.extend_from_slice(&data),
                    }
                }
                Some(Err(e)) => return Err(H2PreviewError::ReadDataFailed(e)),
                None => return Ok(()),
            }
        }
    }

    pub async fn icap_write_preview_data<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        const END_SLICE: &[u8] = b"\r\n0\r\n\r\n";

        let header = format!("{:x}\r\n", self.buffer.len());

        writer
            .write_all_vectored([
                IoSlice::new(header.as_bytes()),
                IoSlice::new(&self.buffer),
                IoSlice::new(END_SLICE),
            ])
            .await?;

        Ok(())
    }

    pub async fn icap_write_all_as_chunked<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        const END_SLICE: &[u8] = b"\r\n0\r\n";

        if self.received == 0 {
            writer.write_all(b"0\r\n").await?;
        } else {
            let header = format!("{:x}\r\n", self.received);

            if let Some(left) = &self.left {
                writer
                    .write_all_vectored([
                        IoSlice::new(header.as_bytes()),
                        IoSlice::new(&self.buffer),
                        IoSlice::new(left),
                        IoSlice::new(END_SLICE),
                    ])
                    .await?;
            } else {
                writer
                    .write_all_vectored([
                        IoSlice::new(header.as_bytes()),
                        IoSlice::new(&self.buffer),
                        IoSlice::new(END_SLICE),
                    ])
                    .await?;
            }
        }

        Ok(())
    }

    pub fn h2_unbounded_send_all(
        mut self,
        send_stream: &mut SendStream<Bytes>,
    ) -> Result<(), h2::Error> {
        send_stream.send_data(self.buffer.into(), false)?;
        if let Some(left) = self.left.take() {
            send_stream.send_data(left, false)?;
        }
        Ok(())
    }
}
