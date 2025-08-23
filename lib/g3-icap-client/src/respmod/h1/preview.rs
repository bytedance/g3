/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::io::{IoSlice, Write};
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use g3_http::{ChunkedDataDecodeReader, H1BodyToChunkedTransfer, HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedWriteExt, StreamCopy, StreamCopyError};

use super::{
    BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse, H1RespmodAdaptationError,
    HttpResponseAdapter, HttpResponseClientWriter, HttpResponseForAdaptation,
    RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::h1::HttpRequestForAdaptation;
use crate::respmod::IcapRespmodResponsePayload;
use crate::respmod::response::RespmodResponse;

impl<I: IdleCheck> HttpResponseAdapter<I> {
    fn build_preview_request(
        &self,
        http_req_hdr_len: usize,
        http_rsp_hdr_len: usize,
        preview_size: usize,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let body_offset = http_req_hdr_len + http_rsp_hdr_len;
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, res-body={body_offset}\r\nPreview: {preview_size}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn xfer_with_preview<R, H, UR, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        http_request: &R,
        http_response: &H,
        ups_body_type: HttpBodyType,
        ups_body_io: &mut UR,
        clt_writer: &mut CW,
        preview_size: usize,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        R: HttpRequestForAdaptation,
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let mut left_chunk_size = 0;
        let preview_buf: Vec<u8>;
        let ups_body_type = match ups_body_type {
            HttpBodyType::ReadUntilEnd => {
                let mut ups_body_reader = HttpBodyReader::new_read_until_end(ups_body_io);
                match self
                    .read_plain_preview_data(
                        &mut ups_body_reader,
                        preview_size,
                        self.icap_client.config.preview_data_read_timeout,
                    )
                    .await
                {
                    Ok(Some(buf)) => preview_buf = buf,
                    Ok(None) => {
                        return self
                            .xfer_without_preview(
                                state,
                                http_request,
                                http_response,
                                ups_body_type,
                                ups_body_io,
                                clt_writer,
                            )
                            .await;
                    }
                    Err(e) => return Err(e),
                }
                if ups_body_reader.finished() {
                    if preview_buf.is_empty() {
                        state.mark_ups_recv_no_body();
                        return self
                            .xfer_without_body(state, http_request, http_response, clt_writer)
                            .await;
                    } else {
                        state.mark_ups_recv_all();
                        return self
                            .xfer_small_body(
                                state,
                                http_request,
                                http_response,
                                preview_buf,
                                clt_writer,
                            )
                            .await;
                    }
                }

                HttpBodyType::ReadUntilEnd
            }
            HttpBodyType::ContentLength(n) => {
                let mut ups_body_reader = HttpBodyReader::new_fixed_length(ups_body_io, n);
                match self
                    .read_plain_preview_data(
                        &mut ups_body_reader,
                        preview_size,
                        self.icap_client.config.preview_data_read_timeout,
                    )
                    .await
                {
                    Ok(Some(buf)) => preview_buf = buf,
                    Ok(None) => {
                        return self
                            .xfer_without_preview(
                                state,
                                http_request,
                                http_response,
                                ups_body_type,
                                ups_body_io,
                                clt_writer,
                            )
                            .await;
                    }
                    Err(e) => return Err(e),
                }
                if ups_body_reader.finished() {
                    state.mark_ups_recv_all();
                    return self
                        .xfer_small_body(
                            state,
                            http_request,
                            http_response,
                            preview_buf,
                            clt_writer,
                        )
                        .await;
                }

                HttpBodyType::ContentLength(n - (preview_buf.len() as u64))
            }
            HttpBodyType::Chunked => {
                let mut ups_body_reader =
                    ChunkedDataDecodeReader::new(ups_body_io, self.http_body_line_max_size);
                match self
                    .read_chunked_preview_data(
                        &mut ups_body_reader,
                        preview_size,
                        self.icap_client.config.preview_data_read_timeout,
                    )
                    .await
                {
                    Ok(Some(buf)) => preview_buf = buf,
                    Ok(None) => {
                        return self
                            .xfer_without_preview(
                                state,
                                http_request,
                                http_response,
                                ups_body_type,
                                ups_body_io,
                                clt_writer,
                            )
                            .await;
                    }
                    Err(e) => return Err(e),
                }
                if ups_body_reader.finished() {
                    let trailer_reader =
                        HttpBodyReader::new_trailer(ups_body_io, self.http_body_line_max_size);
                    return self
                        .xfer_small_body_chunked(
                            state,
                            http_request,
                            http_response,
                            preview_buf,
                            trailer_reader,
                            clt_writer,
                        )
                        .await;
                }
                left_chunk_size = ups_body_reader.left_chunk_size().ok_or_else(|| {
                    H1RespmodAdaptationError::InternalServerError(
                        "broken chunked encoding after preview read",
                    )
                })?;

                HttpBodyType::Chunked
            }
        };

        self.send_preview_data(http_request, http_response, &preview_buf)
            .await?;

        let rsp = RespmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            100 => {
                let mut body_transfer = match ups_body_type {
                    HttpBodyType::ReadUntilEnd => H1BodyToChunkedTransfer::new_read_until_end(
                        ups_body_io,
                        &mut self.icap_connection.writer,
                        self.copy_config,
                    ),
                    HttpBodyType::ContentLength(len) => H1BodyToChunkedTransfer::new_fixed_length(
                        ups_body_io,
                        &mut self.icap_connection.writer,
                        len,
                        self.copy_config,
                    ),
                    HttpBodyType::Chunked => H1BodyToChunkedTransfer::new_chunked_after_preview(
                        ups_body_io,
                        &mut self.icap_connection.writer,
                        left_chunk_size,
                        self.http_body_line_max_size,
                        self.copy_config,
                    ),
                };
                let bidirectional_transfer = BidirectionalRecvIcapResponse {
                    icap_client: &self.icap_client,
                    icap_reader: &mut self.icap_connection.reader,
                    idle_checker: &self.idle_checker,
                };
                let rsp = bidirectional_transfer
                    .transfer_and_recv(&mut body_transfer)
                    .await?;
                if body_transfer.finished() {
                    state.mark_ups_recv_all();
                }

                match rsp.code {
                    204 | 206 => {
                        return Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                            IcapErrorReason::InvalidResponseAfterContinue,
                            rsp.code,
                            rsp.reason,
                        ));
                    }
                    n if (200..300).contains(&n) => {}
                    _ => {
                        return Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                            IcapErrorReason::UnknownResponseAfterContinue,
                            rsp.code,
                            rsp.reason,
                        ));
                    }
                }
                match rsp.payload {
                    IcapRespmodResponsePayload::NoPayload => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.icap_connection.mark_reader_finished();
                        self.handle_icap_ok_without_payload(rsp).await
                    }
                    IcapRespmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.handle_icap_http_response_without_body(
                            state,
                            rsp,
                            header_size,
                            http_response,
                            clt_writer,
                        )
                        .await
                    }
                    IcapRespmodResponsePayload::HttpResponseWithBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                            self.handle_icap_http_response_with_body_after_transfer(
                                state,
                                rsp,
                                header_size,
                                http_response,
                                clt_writer,
                            )
                            .await
                        } else {
                            let icap_keepalive = rsp.keep_alive;
                            let mut bidirectional_transfer = BidirectionalRecvHttpResponse {
                                http_body_line_max_size: self.http_body_line_max_size,
                                copy_config: self.copy_config,
                                idle_checker: &self.idle_checker,
                                http_header_size: header_size,
                                icap_read_finished: false,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    http_response,
                                    &mut self.icap_connection.reader,
                                    clt_writer,
                                )
                                .await?;
                            if body_transfer.finished() {
                                state.mark_ups_recv_all();
                                self.icap_connection.mark_writer_finished();
                                if bidirectional_transfer.icap_read_finished {
                                    self.icap_connection.mark_reader_finished();
                                    if icap_keepalive {
                                        self.icap_client.save_connection(self.icap_connection);
                                    }
                                }
                            }
                            Ok(r)
                        }
                    }
                }
            }
            204 => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                }

                state.mark_clt_send_start();
                clt_writer
                    .send_response_header(http_response)
                    .await
                    .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
                state.mark_clt_send_header();

                match ups_body_type {
                    HttpBodyType::ReadUntilEnd | HttpBodyType::ContentLength(_) => {
                        self.send_original_plain_body_to_client(
                            rsp,
                            ups_body_type,
                            ups_body_io,
                            clt_writer,
                            preview_buf,
                        )
                        .await?;
                    }
                    HttpBodyType::Chunked => {
                        self.send_original_chunked_body_to_client(
                            rsp,
                            ups_body_io,
                            clt_writer,
                            preview_buf,
                            left_chunk_size,
                        )
                        .await?;
                    }
                }

                state.mark_ups_recv_all();
                state.mark_clt_send_all();

                Ok(RespmodAdaptationEndState::OriginalTransferred)
            }
            206 => Err(H1RespmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                // FIXME we should stop send the pending HTTP body to ICAP server?
                self.icap_connection.mark_writer_finished();
                match rsp.payload {
                    IcapRespmodResponsePayload::NoPayload => {
                        self.icap_connection.mark_reader_finished();
                        self.handle_icap_ok_without_payload(rsp).await
                    }
                    IcapRespmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                        self.handle_icap_http_response_without_body(
                            state,
                            rsp,
                            header_size,
                            http_response,
                            clt_writer,
                        )
                        .await
                    }
                    IcapRespmodResponsePayload::HttpResponseWithBody(header_size) => {
                        self.handle_icap_http_response_with_body_after_transfer(
                            state,
                            rsp,
                            header_size,
                            http_response,
                            clt_writer,
                        )
                        .await
                    }
                }
            }
            _ => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponseForPreview,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }

    async fn read_plain_preview_data<R>(
        &mut self,
        reader: &mut R,
        max_size: usize,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, H1RespmodAdaptationError>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = vec![0u8; max_size];
        let mut read_offset;
        match tokio::time::timeout(timeout, reader.read(&mut buf)).await {
            Ok(Ok(n)) => read_offset = n,
            Ok(Err(e)) => return Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
            Err(_) => return Ok(None),
        }

        let mut pin_reader = Pin::new(reader);
        while read_offset < max_size {
            let mut read_buf = ReadBuf::new(&mut buf[read_offset..]);
            match poll_fn(
                |cx| match pin_reader.as_mut().poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(_)) => Poll::Ready(Ok(Some(read_buf.filled().len()))),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Ready(Ok(None)),
                },
            )
            .await
            {
                Ok(Some(0)) => break,
                Ok(Some(n)) => read_offset += n,
                Ok(None) => break,
                Err(e) => return Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
            }
        }

        buf.truncate(read_offset);
        Ok(Some(buf))
    }

    async fn read_chunked_preview_data<R>(
        &mut self,
        reader: &mut ChunkedDataDecodeReader<'_, R>,
        max_size: usize,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, H1RespmodAdaptationError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut buf = vec![0u8; max_size];
        let mut read_offset;
        match tokio::time::timeout(timeout, reader.read(&mut buf)).await {
            Ok(Ok(n)) => read_offset = n,
            Ok(Err(e)) => return Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
            Err(_) => return Ok(None),
        }

        let mut pin_reader = Pin::new(reader);
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;
        let mut is_active = false;

        while read_offset < max_size {
            let mut read_buf = ReadBuf::new(&mut buf[read_offset..]);

            let pin_read = poll_fn(
                |cx| match pin_reader.as_mut().poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(_)) => Poll::Ready(Ok(Some(read_buf.filled().len()))),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => {
                        if pin_reader.pending_cancel_safe() {
                            Poll::Ready(Ok(None))
                        } else {
                            Poll::Pending
                        }
                    }
                },
            );

            tokio::select! {
                biased;

                r = pin_read => {
                    match r {
                        Ok(Some(0)) => break,
                        Ok(Some(n)) => {
                            is_active = true;
                            read_offset += n;
                        },
                        Ok(None) => break,
                        Err(e) => return Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                    }
                }
                n = idle_interval.tick() => {
                    if !is_active {
                        idle_count += n;

                        if self.idle_checker.check_quit(idle_count) {
                            return Err(H1RespmodAdaptationError::HttpUpstreamReadIdle);
                        }
                    } else {
                        idle_count = 0;
                        is_active = false;
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        buf.truncate(read_offset);
        Ok(Some(buf))
    }

    async fn send_preview_data<R, H>(
        &mut self,
        http_request: &R,
        http_response: &H,
        data: &[u8],
    ) -> Result<(), H1RespmodAdaptationError>
    where
        R: HttpRequestForAdaptation,
        H: HttpResponseForAdaptation,
    {
        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();

        let icap_header =
            self.build_preview_request(http_req_header.len(), http_rsp_header.len(), data.len());

        let chunk_start = format!("{:x}\r\n", data.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_req_header),
                IoSlice::new(&http_rsp_header),
                IoSlice::new(chunk_start.as_bytes()),
                IoSlice::new(data),
                IoSlice::new(b"\r\n0\r\n\r\n"),
            ])
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)
    }

    async fn send_original_plain_body_to_client<CR, UW>(
        self,
        icap_rsp: RespmodResponse,
        ups_body_type: HttpBodyType,
        ups_body_io: &mut CR,
        clt_writer: &mut UW,
        preview_buf: Vec<u8>,
    ) -> Result<(), H1RespmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        clt_writer
            .write_all(&preview_buf)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;

        let mut clt_body_reader =
            HttpBodyReader::new(ups_body_io, ups_body_type, self.http_body_line_max_size);
        let mut body_copy = StreamCopy::new(&mut clt_body_reader, clt_writer, &self.copy_config);

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1RespmodAdaptationError::HttpUpstreamReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    async fn send_original_chunked_body_to_client<CR, UW>(
        self,
        icap_rsp: RespmodResponse,
        ups_body_io: &mut CR,
        clt_writer: &mut UW,
        preview_buf: Vec<u8>,
        left_chunk_size: u64,
    ) -> Result<(), H1RespmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let chunk_header = format!("{:x}\r\n", preview_buf.len());
        clt_writer
            .write_all_vectored([
                IoSlice::new(chunk_header.as_bytes()),
                IoSlice::new(&preview_buf),
                IoSlice::new(b"\r\n"),
            ])
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;

        let mut chunked_transfer = H1BodyToChunkedTransfer::new_chunked_after_preview(
            ups_body_io,
            clt_writer,
            left_chunk_size,
            self.http_body_line_max_size,
            self.copy_config,
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut chunked_transfer => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if chunked_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if chunked_transfer.no_cached_data() {
                                Err(H1RespmodAdaptationError::HttpUpstreamReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        chunked_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
