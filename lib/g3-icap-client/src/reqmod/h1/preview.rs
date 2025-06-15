/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_http::{ChunkedDataDecodeReader, H1BodyToChunkedTransfer, HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedWriteExt, StreamCopy, StreamCopyError};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H1ReqmodAdaptationError,
    HttpRequestAdapter, HttpRequestForAdaptation, HttpRequestUpstreamWriter,
    ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> HttpRequestAdapter<I> {
    fn build_preview_request(&self, http_header_len: usize, preview_size: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={}\r\nPreview: {}\r\n",
            http_header_len, preview_size
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_with_preview<H, CR, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: &H,
        clt_body_type: HttpBodyType,
        clt_body_io: &mut CR,
        ups_writer: &mut UW,
        preview_size: usize,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        CR: AsyncBufRead + Unpin,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let mut left_chunk_size = 0;
        let preview_buf: Vec<u8>;
        let clt_body_type = match clt_body_type {
            HttpBodyType::ReadUntilEnd => {
                let mut clt_body_reader = HttpBodyReader::new_read_until_end(clt_body_io);
                preview_buf = self
                    .read_preview_data(&mut clt_body_reader, preview_size)
                    .await?;
                if preview_buf.is_empty() {
                    state.clt_read_finished = true;
                    return self
                        .xfer_without_body(state, http_request, ups_writer)
                        .await;
                }
                if clt_body_reader.finished() {
                    state.clt_read_finished = true;
                    return self
                        .xfer_small_body(state, http_request, preview_buf, ups_writer)
                        .await;
                }

                HttpBodyType::ReadUntilEnd
            }
            HttpBodyType::ContentLength(n) => {
                let mut clt_body_reader = HttpBodyReader::new_fixed_length(clt_body_io, n);
                preview_buf = self
                    .read_preview_data(&mut clt_body_reader, preview_size)
                    .await?;
                if clt_body_reader.finished() {
                    state.clt_read_finished = true;
                    return self
                        .xfer_small_body(state, http_request, preview_buf, ups_writer)
                        .await;
                }

                HttpBodyType::ContentLength(n - (preview_buf.len() as u64))
            }
            HttpBodyType::Chunked => {
                let mut clt_body_decoder =
                    ChunkedDataDecodeReader::new(clt_body_io, self.http_body_line_max_size);
                preview_buf = self
                    .read_preview_data(&mut clt_body_decoder, preview_size)
                    .await?;
                if clt_body_decoder.finished() {
                    let trailer_reader =
                        HttpBodyReader::new_trailer(clt_body_io, self.http_body_line_max_size);
                    return self
                        .xfer_small_body_chunked(
                            state,
                            http_request,
                            preview_buf,
                            trailer_reader,
                            ups_writer,
                        )
                        .await;
                }
                left_chunk_size = clt_body_decoder.left_chunk_size().ok_or_else(|| {
                    H1ReqmodAdaptationError::InternalServerError(
                        "broken chunked encoding after preview read",
                    )
                })?;

                HttpBodyType::Chunked
            }
        };

        self.send_preview_data(http_request, &preview_buf).await?;

        let mut rsp = ReqmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
        }

        match rsp.code {
            100 => {
                let mut body_transfer = match clt_body_type {
                    HttpBodyType::ReadUntilEnd => H1BodyToChunkedTransfer::new_read_until_end(
                        clt_body_io,
                        &mut self.icap_connection.writer,
                        self.copy_config,
                    ),
                    HttpBodyType::ContentLength(len) => H1BodyToChunkedTransfer::new_fixed_length(
                        clt_body_io,
                        &mut self.icap_connection.writer,
                        len,
                        self.copy_config,
                    ),
                    HttpBodyType::Chunked => H1BodyToChunkedTransfer::new_chunked_after_preview(
                        clt_body_io,
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
                    state.clt_read_finished = true;
                }

                match rsp.code {
                    204 | 206 => {
                        return Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                            IcapErrorReason::InvalidResponseAfterContinue,
                            rsp.code,
                            rsp.reason,
                        ));
                    }
                    n if (200..300).contains(&n) => {}
                    _ => {
                        return Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                            IcapErrorReason::UnknownResponseAfterContinue,
                            rsp.code,
                            rsp.reason,
                        ));
                    }
                }
                match rsp.payload {
                    IcapReqmodResponsePayload::NoPayload => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.icap_connection.mark_reader_finished();
                        self.handle_icap_ok_without_payload(rsp).await
                    }
                    IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.handle_icap_http_request_without_body(
                            state,
                            rsp,
                            header_size,
                            http_request,
                            ups_writer,
                        )
                        .await
                    }
                    IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                            self.handle_icap_http_request_with_body_after_transfer(
                                state,
                                rsp,
                                header_size,
                                http_request,
                                ups_writer,
                            )
                            .await
                        } else {
                            let mut bidirectional_transfer = BidirectionalRecvHttpRequest {
                                http_body_line_max_size: self.http_body_line_max_size,
                                http_req_add_no_via_header: self.http_req_add_no_via_header,
                                copy_config: self.copy_config,
                                idle_checker: &self.idle_checker,
                                http_header_size: header_size,
                                icap_read_finished: false,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    http_request,
                                    &mut self.icap_connection.reader,
                                    ups_writer,
                                )
                                .await?;
                            if body_transfer.finished() {
                                state.clt_read_finished = true;
                                self.icap_connection.mark_writer_finished();
                                if bidirectional_transfer.icap_read_finished {
                                    self.icap_connection.mark_reader_finished();
                                    if rsp.keep_alive {
                                        self.icap_client.save_connection(self.icap_connection);
                                    }
                                }
                            }
                            Ok(r)
                        }
                    }
                    IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.handle_icap_http_response_without_body(rsp, header_size)
                            .await
                            .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None))
                    }
                    IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => {
                        if body_transfer.finished() {
                            self.icap_connection.mark_writer_finished();
                        }
                        self.handle_icap_http_response_with_body(rsp, header_size)
                            .await
                            .map(|(rsp, body)| {
                                ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))
                            })
                    }
                }
            }
            204 => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                }

                ups_writer
                    .send_request_header(http_request)
                    .await
                    .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
                state.mark_ups_send_header();

                match clt_body_type {
                    HttpBodyType::ReadUntilEnd | HttpBodyType::ContentLength(_) => {
                        self.send_original_plain_body_to_upstream(
                            rsp,
                            clt_body_type,
                            clt_body_io,
                            ups_writer,
                            preview_buf,
                        )
                        .await?;
                    }
                    HttpBodyType::Chunked => {
                        self.send_original_chunked_body_to_upstream(
                            rsp,
                            clt_body_io,
                            ups_writer,
                            preview_buf,
                            left_chunk_size,
                        )
                        .await?;
                    }
                }

                state.mark_ups_send_all();
                state.clt_read_finished = true;

                Ok(ReqmodAdaptationEndState::OriginalTransferred)
            }
            206 => Err(H1ReqmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                // FIXME we should stop send the pending HTTP body to ICAP server?
                self.icap_connection.mark_writer_finished();
                match rsp.payload {
                    IcapReqmodResponsePayload::NoPayload => {
                        self.icap_connection.mark_reader_finished();
                        self.handle_icap_ok_without_payload(rsp).await
                    }
                    IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                        self.handle_icap_http_request_without_body(
                            state,
                            rsp,
                            header_size,
                            http_request,
                            ups_writer,
                        )
                        .await
                    }
                    IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                        self.handle_icap_http_request_with_body_after_transfer(
                            state,
                            rsp,
                            header_size,
                            http_request,
                            ups_writer,
                        )
                        .await
                    }
                    IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => self
                        .handle_icap_http_response_without_body(rsp, header_size)
                        .await
                        .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None)),
                    IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => self
                        .handle_icap_http_response_with_body(rsp, header_size)
                        .await
                        .map(|(rsp, body)| {
                            ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))
                        }),
                }
            }
            _ => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponseForPreview,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }

    async fn read_preview_data<R>(
        &mut self,
        reader: &mut R,
        max_size: usize,
    ) -> Result<Vec<u8>, H1ReqmodAdaptationError>
    where
        R: AsyncRead + Unpin,
    {
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut is_active = false;
        let mut idle_count = 0;

        let mut buf = vec![0u8; max_size];
        let mut read_offset = 0;

        while read_offset < max_size {
            tokio::select! {
                biased;

                r = reader.read(&mut buf[read_offset..]) => {
                    match r {
                        Ok(0) => break,
                        Ok(n) => {
                            is_active = true;
                            read_offset += n;
                        }
                        Err(e) => {
                            return Err(H1ReqmodAdaptationError::HttpClientReadFailed(e));
                        }
                    }
                }
                n = idle_interval.tick() => {
                    if !is_active {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return Err(H1ReqmodAdaptationError::HttpClientReadIdle);
                        }
                    } else {
                        idle_count = 0;
                        is_active = false;
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        buf.truncate(read_offset);
        Ok(buf)
    }

    async fn send_preview_data<H>(
        &mut self,
        http_request: &H,
        data: &[u8],
    ) -> Result<(), H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
    {
        let http_header = http_request.serialize_for_adapter();
        let header_len = http_header.len();
        let icap_header = self.build_preview_request(header_len, data.len());

        let chunk_start = format!("{:x}\r\n", data.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_header),
                IoSlice::new(chunk_start.as_bytes()),
                IoSlice::new(data),
                IoSlice::new(b"\r\n0\r\n\r\n"),
            ])
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)
    }

    async fn send_original_plain_body_to_upstream<CR, UW>(
        self,
        icap_rsp: ReqmodResponse,
        clt_body_type: HttpBodyType,
        clt_body_io: &mut CR,
        ups_writer: &mut UW,
        preview_buf: Vec<u8>,
    ) -> Result<(), H1ReqmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        ups_writer
            .write_all(&preview_buf)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;

        let mut clt_body_reader =
            HttpBodyReader::new(clt_body_io, clt_body_type, self.http_body_line_max_size);
        let mut body_copy = StreamCopy::new(&mut clt_body_reader, ups_writer, &self.copy_config);

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::HttpClientReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    async fn send_original_chunked_body_to_upstream<CR, UW>(
        self,
        icap_rsp: ReqmodResponse,
        clt_body_io: &mut CR,
        ups_writer: &mut UW,
        preview_buf: Vec<u8>,
        left_chunk_size: u64,
    ) -> Result<(), H1ReqmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let chunk_header = format!("{:x}\r\n", preview_buf.len());
        ups_writer
            .write_all_vectored([
                IoSlice::new(chunk_header.as_bytes()),
                IoSlice::new(&preview_buf),
                IoSlice::new(b"\r\n"),
            ])
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;

        let mut chunked_transfer = H1BodyToChunkedTransfer::new_chunked_after_preview(
            clt_body_io,
            ups_writer,
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
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::HttpClientReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if chunked_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if chunked_transfer.no_cached_data() {
                                Err(H1ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        chunked_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
