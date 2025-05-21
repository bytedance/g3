/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use bytes::Bytes;
use h2::RecvStream;
use h2::client::{ResponseFuture, SendRequest};
use http::{Request, Response};

use g3_h2::{
    H2BodyTransfer, H2StreamBodyTransferError, H2StreamFromChunkedTransfer,
    H2StreamFromChunkedTransferError, RequestExt,
};
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::IdleCheck;

use super::{
    H2ReqmodAdaptationError, H2RequestAdapter, PreviewData, ReqmodAdaptationEndState,
    ReqmodAdaptationMidState, ReqmodAdaptationRunState,
};
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    pub(super) async fn handle_original_http_request_without_body(
        self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let (ups_recv_rsp, _) = ups_send_request
            .send_request(http_request, true)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();

        let ups_rsp =
            recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout)
                .await?;
        state.mark_ups_recv_header();

        Ok(ReqmodAdaptationEndState::OriginalTransferred(ups_rsp))
    }

    pub(super) async fn handle_original_http_request_with_body(
        self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_request: Request<()>,
        preview_data: PreviewData,
        clt_body: RecvStream,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let (mut ups_recv_rsp, mut ups_send_stream) = ups_send_request
            .send_request(http_request, false)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();

        // no reserve of capacity, let the driver buffer it
        preview_data
            .h2_unbounded_send(&mut ups_send_stream)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed)?;

        let mut body_transfer =
            H2BodyTransfer::new(clt_body, ups_send_stream, self.copy_config.yield_size());

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        fn convert_transfer_error(e: H2StreamBodyTransferError) -> H2ReqmodAdaptationError {
            match e {
                H2StreamBodyTransferError::RecvDataFailed(e)
                | H2StreamBodyTransferError::RecvTrailersFailed(e)
                | H2StreamBodyTransferError::ReleaseRecvCapacityFailed(e) => {
                    H2ReqmodAdaptationError::HttpClientRecvDataFailed(e)
                }
                H2StreamBodyTransferError::SendDataFailed(e)
                | H2StreamBodyTransferError::SendTrailersFailed(e)
                | H2StreamBodyTransferError::WaitSendCapacityFailed(e)
                | H2StreamBodyTransferError::GracefulCloseError(e) => {
                    H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)
                }
                H2StreamBodyTransferError::SenderNotInSendState => {
                    H2ReqmodAdaptationError::HttpUpstreamNotInSendState
                }
            }
        }

        loop {
            tokio::select! {
                biased;

                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            Ok(ReqmodAdaptationEndState::OriginalTransferred(ups_rsp))
                        }
                        Err(e) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e)),
                    };
                }
                r = &mut body_transfer => {
                    match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            break;
                        }
                        Err(e) => return Err(convert_transfer_error(e)),
                    }
                }
                n = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H2ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H2ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        let ups_rsp =
            recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout)
                .await?;
        state.mark_ups_recv_header();

        Ok(ReqmodAdaptationEndState::OriginalTransferred(ups_rsp))
    }

    pub(super) async fn recv_icap_http_request_without_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: Request<()>,
    ) -> Result<ReqmodAdaptationMidState, H2ReqmodAdaptationError> {
        let http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.reader,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let final_req = orig_http_request.adapt_to(&http_req);
        Ok(ReqmodAdaptationMidState::AdaptedRequest(
            http_req, final_req,
        ))
    }

    pub(super) async fn handle_icap_http_request_without_body(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.reader,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let final_req = orig_http_request.adapt_to(&http_req);

        let (ups_recv_rsp, _) = ups_send_request
            .send_request(final_req, true)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();

        let ups_rsp =
            recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout)
                .await?;
        state.mark_ups_recv_header();

        Ok(ReqmodAdaptationEndState::AdaptedTransferred(
            http_req, ups_rsp,
        ))
    }

    pub(super) async fn handle_icap_http_request_with_body_after_transfer(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.reader,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;

        let final_req = orig_http_request.adapt_to(&http_req);
        let (mut ups_recv_rsp, mut ups_send_stream) = ups_send_request
            .send_request(final_req, false)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();

        let mut body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.reader,
            &mut ups_send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            if body_transfer.finished() {
                                self.icap_connection.mark_reader_finished();
                                if icap_rsp.keep_alive {
                                    self.icap_client.save_connection(self.icap_connection);
                                }
                            }
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                        }
                        Err(e) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e)),
                    };
                }
                r = &mut body_transfer => {
                    match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            self.icap_connection.mark_reader_finished();
                            if icap_rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection);
                            }
                            break;
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => return Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => return Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => return Err(H2ReqmodAdaptationError::HttpUpstreamSendTrailedFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => return Err(H2ReqmodAdaptationError::HttpUpstreamNotInSendState),
                    }
                }
                n = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H2ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H2ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        let ups_rsp =
            recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout)
                .await?;
        state.mark_ups_recv_header();

        Ok(ReqmodAdaptationEndState::AdaptedTransferred(
            http_req, ups_rsp,
        ))
    }
}

pub(super) async fn recv_ups_response_head_after_transfer(
    response_fut: ResponseFuture,
    timeout: Duration,
) -> Result<Response<RecvStream>, H2ReqmodAdaptationError> {
    match tokio::time::timeout(timeout, response_fut).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e)),
        Err(_) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseTimeout),
    }
}
