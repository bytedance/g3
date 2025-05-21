/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use h2::client::SendRequest;
use http::Request;

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

impl<I: IdleCheck> BidirectionalRecvIcapResponse<'_, I> {
    pub(super) async fn transfer_and_recv(
        self,
        mut body_transfer: &mut H2StreamToChunkedTransfer<'_, IcapClientWriter>,
    ) -> Result<ReqmodResponse, H2ReqmodAdaptationError> {
        let mut idle_interval = self.idle_checker.interval_timer();
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
                n = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += n;

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

    async fn recv_icap_response(self) -> Result<ReqmodResponse, H2ReqmodAdaptationError> {
        let rsp = ReqmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;
        Ok(rsp)
    }
}

pub(super) struct BidirectionalRecvHttpRequest<'a, I: IdleCheck> {
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) http_body_line_max_size: usize,
    pub(super) http_trailer_max_size: usize,
    pub(super) http_rsp_head_recv_timeout: Duration,
    pub(super) http_req_add_no_via_header: bool,
    pub(super) idle_checker: &'a I,
    pub(super) http_header_size: usize,
    pub(super) icap_read_finished: bool,
}

impl<I: IdleCheck> BidirectionalRecvHttpRequest<'_, I> {
    pub(super) async fn transfer(
        &mut self,
        state: &mut ReqmodAdaptationRunState,
        mut clt_body_transfer: &mut H2StreamToChunkedTransfer<'_, IcapClientWriter>,
        orig_http_request: Request<()>,
        mut ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_req = HttpAdaptedRequest::parse(
            self.icap_reader,
            self.http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;

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
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut ups_recv_rsp => {
                    return match r {
                        Ok(ups_rsp) => {
                            state.mark_ups_recv_header();
                            if ups_body_transfer.finished() {
                                self.icap_read_finished = true;
                            }
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
                                    self.icap_read_finished = true;
                                    let ups_rsp = recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout).await?;
                                    Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                                }
                                Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => Err(H2ReqmodAdaptationError::HttpUpstreamNotInSendState),
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
                            self.icap_read_finished = true;
                            let ups_rsp = recv_ups_response_head_after_transfer(ups_recv_rsp, self.http_rsp_head_recv_timeout).await?;
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred(http_req, ups_rsp))
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2ReqmodAdaptationError::HttpUpstreamSendTrailedFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => Err(H2ReqmodAdaptationError::HttpUpstreamNotInSendState),
                    };
                }
                n = idle_interval.tick() => {
                    if clt_body_transfer.is_idle() && ups_body_transfer.is_idle() {
                        idle_count += n;

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
