/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, IoSlice, Write};

use bytes::{BufMut, Bytes};
use h2::client::SendRequest;
use h2::{RecvStream, SendStream};
use http::Request;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_h2::{H2StreamToChunkedTransfer, RequestExt};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H2ReqmodAdaptationError,
    H2RequestAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    fn build_preview_request(&self, http_header_len: usize, preview_size: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header, None);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\nPreview: {preview_size}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_with_preview(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        mut clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
        max_preview_size: usize,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let mut preview_data = PreviewData::new(max_preview_size);
        preview_data.recv(&mut clt_body, &self.idle_checker).await?;

        if preview_data.is_empty() {
            return self
                .xfer_without_preview(state, http_request, clt_body, ups_send_request)
                .await;
        }
        if preview_data.end_of_data {
            return self
                .xfer_small_body(
                    state,
                    http_request,
                    preview_data,
                    clt_body,
                    ups_send_request,
                )
                .await;
        }

        let http_header = http_request.serialize_for_adapter();
        let icap_header =
            self.build_preview_request(http_header.len(), preview_data.preview_size());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        preview_data
            .write_preview_data(icap_w)
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

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
                let mut body_transfer = if let Some(left_data) = preview_data.left.take() {
                    H2StreamToChunkedTransfer::with_chunk(
                        &mut clt_body,
                        &mut self.icap_connection.writer,
                        self.copy_config.yield_size(),
                        left_data,
                    )
                } else {
                    H2StreamToChunkedTransfer::new(
                        &mut clt_body,
                        &mut self.icap_connection.writer,
                        self.copy_config.yield_size(),
                    )
                };

                let bidirectional_transfer = BidirectionalRecvIcapResponse {
                    icap_client: &self.icap_client,
                    icap_reader: &mut self.icap_connection.reader,
                    idle_checker: &self.idle_checker,
                };
                let rsp = bidirectional_transfer
                    .transfer_and_recv(&mut body_transfer)
                    .await?;

                match rsp.code {
                    204 | 206 => {
                        return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                            IcapErrorReason::InvalidResponseAfterContinue,
                            rsp.code,
                            rsp.reason,
                        ));
                    }
                    n if (200..300).contains(&n) => {}
                    _ => {
                        return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
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
                            ups_send_request,
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
                                ups_send_request,
                            )
                            .await
                        } else {
                            let mut bidirectional_transfer = BidirectionalRecvHttpRequest {
                                icap_reader: &mut self.icap_connection.reader,
                                copy_config: self.copy_config,
                                http_body_line_max_size: self.http_body_line_max_size,
                                http_trailer_max_size: self.http_trailer_max_size,
                                http_rsp_head_recv_timeout: self.http_rsp_head_recv_timeout,
                                http_req_add_no_via_header: self.http_req_add_no_via_header,
                                idle_checker: &self.idle_checker,
                                http_header_size: header_size,
                                icap_read_finished: false,
                            };
                            let r = bidirectional_transfer
                                .transfer(state, &mut body_transfer, http_request, ups_send_request)
                                .await?;

                            let icap_read_finished = bidirectional_transfer.icap_read_finished;
                            if body_transfer.finished() {
                                self.icap_connection.mark_writer_finished();
                                if icap_read_finished {
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
                self.handle_original_http_request_with_body(
                    state,
                    rsp,
                    http_request,
                    preview_data,
                    clt_body,
                    ups_send_request,
                )
                .await
            }
            206 => Err(H2ReqmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
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
                            ups_send_request,
                        )
                        .await
                    }
                    IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                        self.handle_icap_http_request_with_body_after_transfer(
                            state,
                            rsp,
                            header_size,
                            http_request,
                            ups_send_request,
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
                Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponseForPreview,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }
}

pub(super) struct PreviewData {
    max_size: usize,
    received: usize,
    buffer: Vec<u8>,
    left: Option<Bytes>,
    end_of_data: bool,
}

impl PreviewData {
    fn new(preview_size: usize) -> Self {
        PreviewData {
            max_size: preview_size,
            received: 0,
            buffer: Vec::with_capacity(preview_size),
            left: None,
            end_of_data: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn preview_size(&self) -> usize {
        self.buffer.len()
    }

    async fn recv<I: IdleCheck>(
        &mut self,
        clt_body: &mut RecvStream,
        idle_checker: &I,
    ) -> Result<(), H2ReqmodAdaptationError> {
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
                            return Err(H2ReqmodAdaptationError::HttpClientRecvDataFailed(e));
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
                            return Err(H2ReqmodAdaptationError::HttpClientReadIdle);
                        }
                    } else {
                        idle_count = 0;
                        is_active = false;
                    }

                    if let Some(reason) = idle_checker.check_force_quit() {
                        return Err(H2ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        self.end_of_data = true;
        Ok(())
    }

    async fn write_preview_data<W>(&self, writer: &mut W) -> io::Result<()>
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

    pub(super) async fn write_all_as_chunked<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        const END_SLICE: &[u8] = b"\r\n0\r\n";

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

        Ok(())
    }

    pub(super) fn h2_unbounded_send(
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
