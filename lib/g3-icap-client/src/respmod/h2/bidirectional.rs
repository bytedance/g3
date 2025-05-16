/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use http::Response;

use g3_h2::{
    H2StreamFromChunkedTransfer, H2StreamFromChunkedTransferError, H2StreamToChunkedTransfer,
    H2StreamToChunkedTransferError, ResponseExt,
};
use g3_io_ext::{IdleCheck, LimitedBufReadExt, LimitedCopyConfig};

use super::{
    H2RespmodAdaptationError, H2SendResponseToClient, HttpAdaptedResponse,
    RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::respmod::response::RespmodResponse;
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
    ) -> Result<RespmodResponse, H2RespmodAdaptationError> {
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => self.recv_icap_response().await,
                        Err(H2StreamToChunkedTransferError::WriteError(e)) => Err(H2RespmodAdaptationError::IcapServerWriteFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvDataFailed(e)) => Err(H2RespmodAdaptationError::HttpUpstreamRecvDataFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpUpstreamRecvTrailerFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(H2RespmodAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(H2RespmodAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H2RespmodAdaptationError::HttpUpstreamReadIdle)
                            } else {
                                Err(H2RespmodAdaptationError::IcapServerWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn recv_icap_response(
        self,
    ) -> Result<RespmodResponse, H2RespmodAdaptationError> {
        let rsp = RespmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(H2RespmodAdaptationError::IcapServerErrorResponse(
                IcapErrorReason::InvalidResponseAfterContinue,
                rsp.code,
                rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(H2RespmodAdaptationError::IcapServerErrorResponse(
                IcapErrorReason::UnknownResponseAfterContinue,
                rsp.code,
                rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpResponse<'a, I: IdleCheck> {
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) http_body_line_max_size: usize,
    pub(super) http_trailer_max_size: usize,
    pub(super) idle_checker: &'a I,
    pub(super) http_header_size: usize,
    pub(super) icap_read_finished: bool,
}

impl<I: IdleCheck> BidirectionalRecvHttpResponse<'_, I> {
    pub(super) async fn transfer<CW>(
        &mut self,
        state: &mut RespmodAdaptationRunState,
        mut ups_body_transfer: &mut H2StreamToChunkedTransfer<'_, IcapClientWriter>,
        orig_http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let http_rsp = HttpAdaptedResponse::parse(self.icap_reader, self.http_header_size).await?;

        let final_rsp = orig_http_response.adapt_to(&http_rsp);
        state.mark_clt_send_start();
        let mut clt_send_stream = clt_send_response
            .send_response(final_rsp, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();

        let mut adp_body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_reader,
            &mut clt_send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut ups_body_transfer => {
                    return match r {
                        Ok(_) => {
                            match adp_body_transfer.await {
                                Ok(_) => {
                                    state.mark_clt_send_all();
                                    self.icap_read_finished = true;
                                    Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
                                }
                                Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2RespmodAdaptationError::IcapServerReadFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendDataFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendTrailerFailed(e)),
                                Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => Err(H2RespmodAdaptationError::HttpClientNotInSendState),
                            }
                        }
                        Err(H2StreamToChunkedTransferError::WriteError(e)) => Err(H2RespmodAdaptationError::IcapServerWriteFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvDataFailed(e)) => Err(H2RespmodAdaptationError::HttpUpstreamRecvDataFailed(e)),
                        Err(H2StreamToChunkedTransferError::RecvTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpUpstreamRecvTrailerFailed(e)),
                    };
                }
                r = &mut adp_body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            self.icap_read_finished = true;
                            Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendTrailerFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => Err(H2RespmodAdaptationError::HttpClientNotInSendState),
                    };
                }
                n = idle_interval.tick() => {
                    if ups_body_transfer.is_idle() && adp_body_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if ups_body_transfer.is_idle() {
                                if ups_body_transfer.no_cached_data() {
                                    Err(H2RespmodAdaptationError::HttpUpstreamReadIdle)
                                } else {
                                    Err(H2RespmodAdaptationError::IcapServerWriteIdle)
                                }
                            } else if adp_body_transfer.no_cached_data() {
                                Err(H2RespmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H2RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        ups_body_transfer.reset_active();
                        adp_body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H2RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
