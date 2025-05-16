/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::Bytes;
use h2::RecvStream;
use http::Response;

use g3_h2::{
    H2BodyTransfer, H2StreamBodyTransferError, H2StreamFromChunkedTransfer,
    H2StreamFromChunkedTransferError, ResponseExt,
};
use g3_http::client::HttpAdaptedResponse;
use g3_io_ext::IdleCheck;

use super::{
    H2RespmodAdaptationError, H2ResponseAdapter, H2SendResponseToClient, RespmodAdaptationEndState,
    RespmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::respmod::response::RespmodResponse;

impl<I: IdleCheck> H2ResponseAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload(
        self,
        icap_rsp: RespmodResponse,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a payload
        Err(H2RespmodAdaptationError::IcapServerErrorResponse(
            IcapErrorReason::NoBodyFound,
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_original_http_response_without_body<CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        state.mark_clt_send_start();
        clt_send_response
            .send_response(http_response, true)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();

        Ok(RespmodAdaptationEndState::OriginalTransferred)
    }

    pub(super) async fn handle_original_http_response_with_body<CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_response: Response<()>,
        initial_body_data: Bytes,
        ups_body: RecvStream,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        state.mark_clt_send_start();
        let mut clt_send_stream = clt_send_response
            .send_response(http_response, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();

        if ups_body.is_end_stream() {
            // no reserve of capacity, let the driver buffer it
            clt_send_stream
                .send_data(initial_body_data, true)
                .map_err(H2RespmodAdaptationError::HttpClientSendDataFailed)?;
            state.mark_clt_send_all();

            return Ok(RespmodAdaptationEndState::OriginalTransferred);
        }

        // no reserve of capacity, let the driver buffer it
        clt_send_stream
            .send_data(initial_body_data, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendDataFailed)?;

        let mut body_transfer =
            H2BodyTransfer::new(ups_body, clt_send_stream, self.copy_config.yield_size());

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        fn convert_transfer_error(e: H2StreamBodyTransferError) -> H2RespmodAdaptationError {
            match e {
                H2StreamBodyTransferError::RecvDataFailed(e)
                | H2StreamBodyTransferError::RecvTrailersFailed(e)
                | H2StreamBodyTransferError::ReleaseRecvCapacityFailed(e) => {
                    H2RespmodAdaptationError::HttpUpstreamRecvDataFailed(e)
                }
                H2StreamBodyTransferError::SendDataFailed(e)
                | H2StreamBodyTransferError::SendTrailersFailed(e)
                | H2StreamBodyTransferError::WaitSendCapacityFailed(e)
                | H2StreamBodyTransferError::GracefulCloseError(e) => {
                    H2RespmodAdaptationError::HttpClientSendDataFailed(e)
                }
                H2StreamBodyTransferError::SenderNotInSendState => {
                    H2RespmodAdaptationError::HttpClientNotInSendState
                }
            }
        }

        loop {
            tokio::select! {
                biased;

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            Ok(RespmodAdaptationEndState::OriginalTransferred)
                        }
                        Err(e) => Err(convert_transfer_error(e)),
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
                                Err(H2RespmodAdaptationError::HttpClientWriteIdle)
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

    pub(super) async fn handle_icap_http_response_without_body<CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_header_size: usize,
        orig_http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let http_rsp =
            HttpAdaptedResponse::parse(&mut self.icap_connection.reader, http_header_size).await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let final_rsp = orig_http_response.adapt_to(&http_rsp);
        state.mark_clt_send_start();
        clt_send_response
            .send_response(final_rsp, true)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();

        Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
    }

    pub(super) async fn handle_icap_http_response_with_body_after_transfer<CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_header_size: usize,
        orig_http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let http_rsp =
            HttpAdaptedResponse::parse(&mut self.icap_connection.reader, http_header_size).await?;

        let final_rsp = orig_http_response.adapt_to(&http_rsp);
        state.mark_clt_send_start();
        let mut clt_send_stream = clt_send_response
            .send_response(final_rsp, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();

        let mut body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.reader,
            &mut clt_send_stream,
            &self.copy_config,
            self.http_body_line_max_size,
            self.http_trailer_max_size,
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            self.icap_connection.mark_reader_finished();
                            if icap_rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection);
                            }
                            Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendTrailerFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SenderNotInSendState) => Err(H2RespmodAdaptationError::HttpClientNotInSendState),
                    };
                }
                n = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H2RespmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H2RespmodAdaptationError::HttpClientWriteIdle)
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
}
