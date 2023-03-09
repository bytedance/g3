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

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use h2::client::SendRequest;
use http::Request;
use tokio::time::Instant;

use g3_h2::{
    H2StreamFromChunkedTransfer, H2StreamFromChunkedTransferError, H2StreamToChunkedTransfer,
    H2StreamToChunkedTransferError, RequestExt,
};
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::{IdleCheck, LimitedBufReadExt, LimitedCopyConfig};

use super::recv_request::recv_ups_response_head_after_transfer;
use super::{H2ReqmodAdaptationError, ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::reqmod::response::ReqmodResponse;
use crate::{IcapClientReader, IcapClientWriter, IcapServiceClient};

pub(super) struct BidirectionalRecvIcapResponse<'a, I: IdleCheck> {
    pub(super) icap_client: &'a Arc<IcapServiceClient>,
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) idle_checker: &'a I,
}

impl<'a, I: IdleCheck> BidirectionalRecvIcapResponse<'a, I> {
    pub(super) async fn transfer_and_recv(
        self,
        mut body_transfer: &mut H2StreamToChunkedTransfer<'_, IcapClientWriter>,
    ) -> Result<ReqmodResponse, H2ReqmodAdaptationError> {
        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => self.recv_icap_response().await,
                        Err(H2StreamToChunkedTransferError::WriteError(e)) => Err(H2ReqmodAdaptationError::IcapServerWriteFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvDataFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvTrailerFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(H2ReqmodAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H2ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H2ReqmodAdaptationError::IcapServerWriteIdle)
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
    }

    pub(super) async fn recv_icap_response(
        self,
    ) -> Result<ReqmodResponse, H2ReqmodAdaptationError> {
        let rsp = ReqmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpRequest<'a, I: IdleCheck> {
    pub(super) icap_rsp: ReqmodResponse,
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) http_body_line_max_size: usize,
    pub(super) http_trailer_max_size: usize,
    pub(super) http_rsp_head_recv_timeout: Duration,
    pub(super) http_req_add_no_via_header: bool,
    pub(super) idle_checker: &'a I,
}

impl<'a, I: IdleCheck> BidirectionalRecvHttpRequest<'a, I> {
    pub(super) async fn transfer(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        mut clt_body_transfer: &mut H2StreamToChunkedTransfer<'_, IcapClientWriter>,
        http_header_size: usize,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let mut http_req = HttpAdaptedRequest::parse(
            self.icap_reader,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        let trailers = self.icap_rsp.take_trailers();
        let has_trailer = !trailers.is_empty();
        http_req.set_trailer(trailers);

        let final_req = orig_http_request.adapt_to(&http_req);
        let (mut ups_recv_rsp, mut ups_send_stream) = ups_send_request
            .send_request(final_req, false)
            .map_err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed)?;
        state.mark_ups_send_header();

        let mut ups_body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_reader,
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
                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                        }
                        Err(e) => Err(H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e)),
                    };
                }
                r = &mut clt_body_transfer => {
                    return match r {
                        Ok(_) => {
                            match ups_body_transfer.await {
                                Ok(_) => {
                                    state.mark_ups_send_all();
                                    state.icap_io_finished = true;
                                    let ups_rsp = recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout).await?;
                                    Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                                }
                                Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed(e)),
                            }
                        }
                        Err(H2StreamToChunkedTransferError::WriteError(e)) => Err(H2ReqmodAdaptationError::IcapServerWriteFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvDataFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpClientRecvTrailerFailed(e)),
                    };
                }
                r = &mut ups_body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            if clt_body_transfer.finished() {
                                state.icap_io_finished = true;
                            }
                            let ups_rsp = recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout).await?;
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendTrailedFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_body_transfer.is_idle() && ups_body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if clt_body_transfer.is_idle() {
                                if clt_body_transfer.no_cached_data() {
                                    Err(H2ReqmodAdaptationError::HttpClientReadIdle)
                                } else {
                                    Err(H2ReqmodAdaptationError::IcapServerWriteIdle)
                                }
                            } else if ups_body_transfer.no_cached_data() {
                                Err(H2ReqmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H2ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_body_transfer.reset_active();
                        ups_body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
