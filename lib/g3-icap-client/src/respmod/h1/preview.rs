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
        preview_state: &PreviewDataState,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, res-body={}\r\nPreview: {}\r\n",
            http_req_hdr_len + http_rsp_hdr_len,
            preview_state.preview_size
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
        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();
        let http_rsp_hdr_len = http_rsp_header.len();
        let preview_data = PreviewData {
            header: Some(http_rsp_header),
            body_type: ups_body_type,
            limit: preview_size,
            inner: ups_body_io,
        };
        let (http_rsp_payload, preview_state) = match tokio::time::timeout(
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
                        http_response,
                        ups_body_type,
                        ups_body_io,
                        clt_writer,
                    )
                    .await;
            }
        };
        let icap_header =
            self.build_preview_request(http_req_header.len(), http_rsp_hdr_len, &preview_state);

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_req_header),
                IoSlice::new(&http_rsp_payload),
            ])
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;

        let rsp = RespmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            100 => {
                if preview_state.preview_eof {
                    return Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                        IcapErrorReason::ContinueAfterPreviewEof,
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let mut body_transfer = H1BodyToChunkedTransfer::new_after_preview(
                    ups_body_io,
                    &mut self.icap_connection.writer,
                    ups_body_type,
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
                    state.mark_ups_recv_all();
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
                self.handle_original_http_response_with_body(
                    state,
                    rsp,
                    http_response,
                    ups_body_io,
                    ups_body_type,
                    clt_writer,
                )
                .await
            }
            206 => Err(H1RespmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                // FIXME we should stop send the pending HTTP body to ICAP server?
                self.icap_connection.mark_writer_finished();
                if preview_state.preview_eof {
                    ups_body_io.consume(preview_state.consume_size);
                    state.mark_ups_recv_all();
                }
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
}
