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

use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite};

use g3_io_ext::{LimitedCopyConfig, LimitedCopyError, ROwnedLimitedCopy};

use super::{ChunkedEncodeTransfer, HttpBodyReader, HttpBodyType, PreviewDataState};

const END_BUFFER: &[u8] = b"\r\n0\r\n\r\n";

pub struct ChunkedTransfer<'a, R, W> {
    body_type: HttpBodyType,
    copy_config: LimitedCopyConfig,
    state: ChunkedTransferState<'a, R, W>,
    total_write: u64,
    active: bool,
}

struct SendHead<'a, R, W> {
    head: String,
    offset: usize,
    body_reader: HttpBodyReader<'a, R>,
    writer: &'a mut W,
}

struct SendEnd<'a, W> {
    offset: usize,
    writer: &'a mut W,
}

enum ChunkedTransferState<'a, R, W> {
    SendHead(SendHead<'a, R, W>),
    Copy(ROwnedLimitedCopy<'a, HttpBodyReader<'a, R>, W>),
    SendEnd(SendEnd<'a, W>),
    Encode(ChunkedEncodeTransfer<'a, R, W>),
    End,
}

impl<'a, R, W> ChunkedTransfer<'a, R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(
        reader: &'a mut R,
        writer: &'a mut W,
        body_type: HttpBodyType,
        body_line_max_len: usize,
        copy_config: LimitedCopyConfig,
    ) -> ChunkedTransfer<'a, R, W> {
        let state = match body_type {
            HttpBodyType::ContentLength(len) => {
                let head = format!("{len:x}\r\n");
                let body_reader = HttpBodyReader::new(
                    reader,
                    HttpBodyType::ContentLength(len),
                    body_line_max_len,
                );
                ChunkedTransferState::SendHead(SendHead {
                    head,
                    offset: 0,
                    body_reader,
                    writer,
                })
            }
            HttpBodyType::ReadUntilEnd => {
                let encoder = ChunkedEncodeTransfer::new(reader, writer, copy_config.yield_size());
                ChunkedTransferState::Encode(encoder)
            }
            HttpBodyType::ChunkedWithoutTrailer | HttpBodyType::ChunkedWithTrailer => {
                let body_reader = HttpBodyReader::new(reader, body_type, body_line_max_len);
                let copy = ROwnedLimitedCopy::new(body_reader, writer, copy_config);
                ChunkedTransferState::Copy(copy)
            }
        };
        ChunkedTransfer {
            body_type,
            copy_config,
            state,
            total_write: 0,
            active: false,
        }
    }

    pub fn new_after_preview(
        reader: &'a mut R,
        writer: &'a mut W,
        body_type: HttpBodyType,
        body_line_max_len: usize,
        copy_config: LimitedCopyConfig,
        preview_state: PreviewDataState,
    ) -> ChunkedTransfer<'a, R, W> {
        let state = match body_type {
            HttpBodyType::ContentLength(len) => {
                let left_len = len - (preview_state.preview_size as u64);
                let head = format!("{left_len:x}\r\n");
                reader.consume(preview_state.consume_size);
                let body_reader = HttpBodyReader::new(
                    reader,
                    HttpBodyType::ContentLength(left_len),
                    body_line_max_len,
                );
                ChunkedTransferState::SendHead(SendHead {
                    head,
                    offset: 0,
                    body_reader,
                    writer,
                })
            }
            HttpBodyType::ReadUntilEnd => {
                reader.consume(preview_state.consume_size);
                let encoder = ChunkedEncodeTransfer::new(reader, writer, copy_config.yield_size());
                ChunkedTransferState::Encode(encoder)
            }
            HttpBodyType::ChunkedWithoutTrailer | HttpBodyType::ChunkedWithTrailer => {
                let next_chunk_size = preview_state.chunked_next_size;
                if next_chunk_size > 0 {
                    let head = format!("{next_chunk_size:x}\r\n");
                    reader.consume(preview_state.consume_size);
                    let body_reader = HttpBodyReader::new_chunked_after_preview(
                        reader,
                        body_type,
                        body_line_max_len,
                        next_chunk_size,
                    );
                    ChunkedTransferState::SendHead(SendHead {
                        head,
                        offset: 0,
                        body_reader,
                        writer,
                    })
                } else {
                    let body_reader = HttpBodyReader::new(reader, body_type, body_line_max_len);
                    let copy = ROwnedLimitedCopy::new(body_reader, writer, copy_config);
                    ChunkedTransferState::Copy(copy)
                }
            }
        };
        ChunkedTransfer {
            body_type,
            copy_config,
            state,
            total_write: 0,
            active: false,
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self.state, ChunkedTransferState::End)
    }

    pub fn is_idle(&self) -> bool {
        !self.active
    }

    pub fn no_cached_data(&self) -> bool {
        match &self.state {
            ChunkedTransferState::SendHead(_) | ChunkedTransferState::SendEnd(_) => false,
            ChunkedTransferState::Copy(copy) => copy.no_cached_data(),
            ChunkedTransferState::Encode(encode) => encode.no_cached_data(),
            ChunkedTransferState::End => true,
        }
    }

    pub fn reset_active(&mut self) {
        match &mut self.state {
            ChunkedTransferState::Copy(copy) => copy.reset_active(),
            ChunkedTransferState::Encode(encode) => encode.reset_active(),
            _ => {}
        }
        self.active = false;
    }
}

impl<'a, R, W> Future for ChunkedTransfer<'a, R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    type Output = Result<(), LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.state {
            ChunkedTransferState::SendHead(send_head) => {
                while send_head.offset < send_head.head.len() {
                    let buf = &send_head.head.as_bytes()[send_head.offset..];
                    let nw = ready!(Pin::new(&mut send_head.writer).poll_write(cx, buf))
                        .map_err(LimitedCopyError::WriteFailed)?;
                    send_head.offset += nw;
                }
                self.total_write += send_head.offset as u64;
                self.active = true;

                let old_state = std::mem::replace(&mut self.state, ChunkedTransferState::End);
                let ChunkedTransferState::SendHead(send_head) = old_state else {
                    unreachable!()
                };
                let copy = ROwnedLimitedCopy::new(
                    send_head.body_reader,
                    send_head.writer,
                    self.copy_config,
                );
                self.state = ChunkedTransferState::Copy(copy);
                self.poll(cx)
            }
            ChunkedTransferState::Copy(copy) => {
                let mut copy = Pin::new(copy);
                match copy.as_mut().poll(cx) {
                    Poll::Pending => {
                        self.active = copy.is_active();
                        return Poll::Pending;
                    }
                    Poll::Ready(Ok(n)) => {
                        self.total_write += n;
                        self.active = true;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                };
                if matches!(self.body_type, HttpBodyType::ContentLength(_)) {
                    let old_state = std::mem::replace(&mut self.state, ChunkedTransferState::End);
                    let ChunkedTransferState::Copy(copy) = old_state else {
                        unreachable!()
                    };
                    self.state = ChunkedTransferState::SendEnd(SendEnd {
                        offset: 0,
                        writer: copy.writer(),
                    });
                    self.poll(cx)
                } else {
                    self.state = ChunkedTransferState::End;
                    Poll::Ready(Ok(()))
                }
            }
            ChunkedTransferState::SendEnd(send_end) => {
                while send_end.offset < END_BUFFER.len() {
                    let buf = &END_BUFFER[send_end.offset..];
                    let nw = ready!(Pin::new(&mut send_end.writer).poll_write(cx, buf))
                        .map_err(LimitedCopyError::WriteFailed)?;
                    send_end.offset += nw;
                }
                self.state = ChunkedTransferState::End;
                self.active = true;
                Poll::Ready(Ok(()))
            }
            ChunkedTransferState::Encode(encode) => {
                let mut encode = Pin::new(encode);
                match encode.as_mut().poll(cx) {
                    Poll::Pending => {
                        self.active = encode.is_active();
                        Poll::Pending
                    }
                    Poll::Ready(Ok(n)) => {
                        self.total_write += n;
                        self.active = true;
                        self.state = ChunkedTransferState::End;
                        Poll::Ready(Ok(()))
                    }
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                }
            }
            ChunkedTransferState::End => Poll::Ready(Ok(())),
        }
    }
}
