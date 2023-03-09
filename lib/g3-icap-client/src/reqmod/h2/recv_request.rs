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

use std::time::Duration;

use bytes::Bytes;
use h2::client::{ResponseFuture, SendRequest};
use h2::RecvStream;
use http::{Request, Response};
use tokio::time::Instant;

use g3_h2::{
    H2BodyTransfer, H2StreamBodyTransferError, H2StreamFromChunkedTransfer,
    H2StreamFromChunkedTransferError, RequestExt,
};
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::IdleCheck;

use super::{
    H2ReqmodAdaptationError, H2RequestAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reqmod::response::ReqmodResponse;
use crate::reqmod::IcapReqmodResponsePayload;

impl<I: IdleCheck> H2RequestAdapter<I> {
    pub(super) async fn handle_original_http_request_without_body(
        self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let (ups_recv_rsp, _) = ups_send_request
            .send_request(http_request, true)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();
        if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
            self.icap_client.save_connection(self.icap_connection).await;
        }

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
        initial_body_data: Bytes,
        clt_body: RecvStream,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let (mut ups_recv_rsp, mut ups_send_stream) = ups_send_request
            .send_request(http_request, false)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();

        // no reserve of capacity, let the driver buffer it
        ups_send_stream
            .send_data(initial_body_data, clt_body.is_end_stream())
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed)?;

        let mut body_transfer =
            H2BodyTransfer::new(clt_body, ups_send_stream, self.copy_config.yield_size());

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
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
            }
        }

        loop {
            tokio::select! {
                biased;

                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(ReqmodAdaptationEndState::OriginalTransferred(ups_rsp))
                        }
                        Err(e) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e)),
                    };
                }
                r = &mut body_transfer => {
                    match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            break;
                        }
                        Err(e) => return Err(convert_transfer_error(e)),
                    }
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

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

    pub(super) async fn handle_icap_http_request_without_body(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.1,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;

        let final_req = orig_http_request.adapt_to(&http_req);
        let (ups_recv_rsp, _) = ups_send_request
            .send_request(final_req, true)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }

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
        mut icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let mut http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.1,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        let trailers = icap_rsp.take_trailers();
        let has_trailer = !trailers.is_empty();
        http_req.set_trailer(trailers);

        let final_req = orig_http_request.adapt_to(&http_req);
        let (mut ups_recv_rsp, mut ups_send_stream) = ups_send_request
            .send_request(final_req, false)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();

        let mut body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.1,
            &mut ups_send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
            has_trailer,
        );

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
                                self.icap_client.save_connection(self.icap_connection).await;
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
                            if icap_rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            break;
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => return Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => return Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => return Err(H2ReqmodAdaptationError::HttpUpstreamSendTrailedFailed(e)),
                    }
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

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
