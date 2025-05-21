/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::{BufMut, Bytes};
use h2::RecvStream;
use h2::client::SendRequest;
use http::Request;

use g3_h2::{H2StreamToChunkedTransfer, H2StreamToChunkedTransferError, RequestExt};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H2ReqmodAdaptationError,
    H2RequestAdapter, PreviewData, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    fn build_forward_all_request(&self, http_header_len: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header, None);
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_small_body(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        preview_data: PreviewData,
        clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_forward_all_request(http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

        preview_data
            .write_all_as_chunked(icap_w)
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        self.recv_send_trailer(clt_body).await?;
        self.icap_connection.mark_writer_finished();

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
            204 | 206 => {
                return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::InvalidResponse,
                    rsp.code,
                    rsp.reason,
                ));
            }
            n if (200..300).contains(&n) => {}
            _ => {
                return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponse,
                    rsp.code,
                    rsp.reason,
                ));
            }
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
                .map(|(rsp, body)| ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))),
        }
    }

    async fn recv_send_trailer(
        &mut self,
        mut clt_body: RecvStream,
    ) -> Result<(), H2ReqmodAdaptationError> {
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        let mut trailer_transfer = H2StreamToChunkedTransfer::without_data(
            &mut clt_body,
            &mut self.icap_connection.writer,
        );

        loop {
            tokio::select! {
                biased;

                r = &mut trailer_transfer => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(H2StreamToChunkedTransferError::WriteError(e)) => Err(H2ReqmodAdaptationError::IcapServerWriteFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvDataFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvTrailerFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if trailer_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if trailer_transfer.no_cached_data() {
                                Err(H2ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H2ReqmodAdaptationError::IcapServerWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        trailer_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn xfer_without_preview(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        mut clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_forward_all_request(http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

        let mut body_transfer = H2StreamToChunkedTransfer::new(
            &mut clt_body,
            &mut self.icap_connection.writer,
            self.copy_config.yield_size(),
        );
        let bidirectional_transfer = BidirectionalRecvIcapResponse {
            icap_client: &self.icap_client,
            icap_reader: &mut self.icap_connection.reader,
            idle_checker: &self.idle_checker,
        };
        let mut rsp = bidirectional_transfer
            .transfer_and_recv(&mut body_transfer)
            .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
        }

        match rsp.code {
            204 | 206 => {
                return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::InvalidResponse,
                    rsp.code,
                    rsp.reason,
                ));
            }
            n if (200..300).contains(&n) => {}
            _ => {
                return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponse,
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
                    .map(|(rsp, body)| ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body)))
            }
        }
    }
}
