/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};

use g3_http::{H1BodyToChunkedTransfer, HttpBodyType, PreviewData, PreviewDataState};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H1ReqmodAdaptationError,
    HttpRequestAdapter, HttpRequestForAdaptation, HttpRequestUpstreamWriter,
    ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> HttpRequestAdapter<I> {
    fn build_preview_request(
        &self,
        http_header_len: usize,
        preview_state: &PreviewDataState,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={}\r\nPreview: {}\r\n",
            http_header_len, preview_state.preview_size
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
        let http_header = http_request.serialize_for_adapter();
        let header_len = http_header.len();
        let preview_data = PreviewData {
            header: Some(http_header),
            body_type: clt_body_type,
            limit: preview_size,
            inner: clt_body_io,
        };
        let (http_payload, preview_state) = match tokio::time::timeout(
            self.icap_client.config.preview_data_read_timeout,
            preview_data,
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                return self
                    .xfer_without_preview(
                        state,
                        http_request,
                        clt_body_type,
                        clt_body_io,
                        ups_writer,
                    )
                    .await;
            }
        };
        let icap_header = self.build_preview_request(header_len, &preview_state);

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_payload)])
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;

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
                if preview_state.preview_eof {
                    return Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                        IcapErrorReason::ContinueAfterPreviewEof,
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let mut body_transfer = H1BodyToChunkedTransfer::new_after_preview(
                    clt_body_io,
                    &mut self.icap_connection.writer,
                    clt_body_type,
                    self.http_body_line_max_size,
                    self.copy_config,
                    preview_state,
                );
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
                self.handle_original_http_request_with_body(
                    state,
                    rsp,
                    http_request,
                    clt_body_io,
                    clt_body_type,
                    ups_writer,
                )
                .await
            }
            206 => Err(H1ReqmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                // FIXME we should stop send the pending HTTP body to ICAP server?
                self.icap_connection.mark_writer_finished();
                if preview_state.preview_eof {
                    clt_body_io.consume(preview_state.consume_size);
                    state.clt_read_finished = true;
                }
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
}
