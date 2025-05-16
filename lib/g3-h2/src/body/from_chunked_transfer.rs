/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use h2::SendStream;
use thiserror::Error;
use tokio::io::AsyncBufRead;

use g3_http::{ChunkedDataDecodeReader, TrailerReadError, TrailerReader};
use g3_io_ext::LimitedCopyConfig;

use super::{H2StreamBodyEncodeTransferError, ROwnedH2BodyEncodeTransfer};

#[derive(Debug, Error)]
pub enum H2StreamFromChunkedTransferError {
    #[error("read error: {0:?}")]
    ReadError(io::Error),
    #[error("send data failed: {0}")]
    SendDataFailed(h2::Error),
    #[error("send trailer failed: {0}")]
    SendTrailerFailed(h2::Error),
    #[error("sender not in send state")]
    SenderNotInSendState,
}

struct TrailerTransfer<'a, R> {
    reader: TrailerReader<'a, R>,
    send_stream: &'a mut SendStream<Bytes>,
}

impl<'a, R> TrailerTransfer<'a, R> {
    fn new(
        reader: &'a mut R,
        send_stream: &'a mut SendStream<Bytes>,
        trailer_max_size: usize,
    ) -> Self {
        TrailerTransfer {
            reader: TrailerReader::new(reader, trailer_max_size),
            send_stream,
        }
    }

    fn is_active(&self) -> bool {
        self.reader.is_active()
    }

    fn reset_active(&mut self) {
        self.reader.reset_active()
    }

    fn no_cached_data(&self) -> bool {
        true
    }
}

impl<R> Future for TrailerTransfer<'_, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = Result<(), H2StreamFromChunkedTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let headers = ready!(Pin::new(&mut self.reader).poll(cx)).map_err(|e| match e {
            TrailerReadError::ReadError(e) => H2StreamFromChunkedTransferError::ReadError(e),
            TrailerReadError::ReadClosed => {
                H2StreamFromChunkedTransferError::ReadError(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed wile reading trailer",
                ))
            }
            TrailerReadError::InvalidHeaderLine(e) => H2StreamFromChunkedTransferError::ReadError(
                io::Error::new(io::ErrorKind::InvalidData, format!("invalid trailer: {e}")),
            ),
            TrailerReadError::HeaderTooLarge => H2StreamFromChunkedTransferError::ReadError(
                io::Error::new(io::ErrorKind::InvalidData, "too large trailer"),
            ),
        })?;
        if headers.is_empty() {
            self.send_stream
                .send_data(Bytes::new(), true)
                .map_err(H2StreamFromChunkedTransferError::SendTrailerFailed)?;
        } else {
            self.send_stream
                .send_trailers(headers.into())
                .map_err(H2StreamFromChunkedTransferError::SendTrailerFailed)?;
        }
        Poll::Ready(Ok(()))
    }
}

enum TransferState<'a, R> {
    Data(ROwnedH2BodyEncodeTransfer<'a, ChunkedDataDecodeReader<'a, R>>),
    Trailer(TrailerTransfer<'a, R>),
    End,
}

pub struct H2StreamFromChunkedTransfer<'a, R> {
    state: TransferState<'a, R>,
    trailer_max_size: usize,
    active: bool,
}

impl<'a, R> H2StreamFromChunkedTransfer<'a, R> {
    pub fn new(
        reader: &'a mut R,
        send_stream: &'a mut SendStream<Bytes>,
        copy_config: &LimitedCopyConfig,
        body_line_max_size: usize,
        trailer_max_size: usize,
    ) -> Self {
        let decoder = ChunkedDataDecodeReader::new(reader, body_line_max_size);
        let encode = ROwnedH2BodyEncodeTransfer::new(decoder, send_stream, copy_config);
        H2StreamFromChunkedTransfer {
            state: TransferState::Data(encode),
            trailer_max_size,
            active: false,
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self.state, TransferState::End)
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        !self.active
    }

    pub fn reset_active(&mut self) {
        self.active = false;
        match &mut self.state {
            TransferState::Data(encode) => encode.reset_active(),
            TransferState::Trailer(transfer) => transfer.reset_active(),
            TransferState::End => {}
        }
    }

    pub fn no_cached_data(&self) -> bool {
        match &self.state {
            TransferState::Data(encode) => encode.no_cached_data(),
            TransferState::Trailer(transfer) => transfer.no_cached_data(),
            TransferState::End => true,
        }
    }
}

impl<R> Future for H2StreamFromChunkedTransfer<'_, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = Result<(), H2StreamFromChunkedTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.state {
            TransferState::End => Poll::Ready(Ok(())),
            TransferState::Trailer(transfer) => {
                let mut transfer = Pin::new(transfer);
                match transfer.as_mut().poll(cx) {
                    Poll::Ready(Ok(())) => {
                        self.active = true;
                        self.state = TransferState::End;
                        Poll::Ready(Ok(()))
                    }
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => {
                        self.active = transfer.is_active();
                        Poll::Pending
                    }
                }
            }
            TransferState::Data(encode) => {
                let mut encode = Pin::new(encode);
                match encode.as_mut().poll(cx) {
                    Poll::Ready(Ok(())) => {
                        self.active = true;
                    }
                    Poll::Ready(Err(H2StreamBodyEncodeTransferError::ReadError(e))) => {
                        return Poll::Ready(Err(H2StreamFromChunkedTransferError::ReadError(e)));
                    }
                    Poll::Ready(Err(H2StreamBodyEncodeTransferError::SendDataFailed(e))) => {
                        return Poll::Ready(Err(H2StreamFromChunkedTransferError::SendDataFailed(
                            e,
                        )));
                    }
                    Poll::Ready(Err(H2StreamBodyEncodeTransferError::SenderNotInSendState)) => {
                        return Poll::Ready(Err(
                            H2StreamFromChunkedTransferError::SenderNotInSendState,
                        ));
                    }
                    Poll::Pending => {
                        self.active = encode.is_active();
                        return Poll::Pending;
                    }
                }

                let old_state = std::mem::replace(&mut self.state, TransferState::End);
                let TransferState::Data(encode) = old_state else {
                    unreachable!()
                };

                let (reader, send_stream) = encode.into_io();
                // read trailer (maybe empty) and send
                self.state = TransferState::Trailer(TrailerTransfer::new(
                    reader.into_reader(),
                    send_stream,
                    self.trailer_max_size,
                ));
                self.poll(cx)
            }
        }
    }
}
