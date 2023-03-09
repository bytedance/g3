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

use bytes::Bytes;
use h2::RecvStream;
use http::Response;
use tokio::time::Instant;

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
use crate::respmod::response::RespmodResponse;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> H2ResponseAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload(
        self,
        icap_rsp: RespmodResponse,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        // there should be a payload
        Err(H2RespmodAdaptationError::IcapServerErrorResponse(
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
        state.mark_clt_send_start();
        clt_send_response
            .send_response(http_response, true)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();

        if icap_rsp.keep_alive && icap_rsp.payload == IcapRespmodResponsePayload::NoPayload {
            self.icap_client.save_connection(self.icap_connection).await;
        }
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
        state.mark_clt_send_start();
        let mut clt_send_stream = clt_send_response
            .send_response(http_response, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();

        // no reserve of capacity, let the driver buffer it
        clt_send_stream
            .send_data(initial_body_data, ups_body.is_end_stream())
            .map_err(H2RespmodAdaptationError::HttpClientSendDataFailed)?;

        let mut body_transfer =
            H2BodyTransfer::new(ups_body, clt_send_stream, self.copy_config.yield_size());

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
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
            }
        }

        loop {
            tokio::select! {
                biased;

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            if icap_rsp.keep_alive && icap_rsp.payload == IcapRespmodResponsePayload::NoPayload {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(RespmodAdaptationEndState::OriginalTransferred)
                        }
                        Err(e) => Err(convert_transfer_error(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

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
            HttpAdaptedResponse::parse(&mut self.icap_connection.1, http_header_size).await?;

        let final_rsp = orig_http_response.adapt_to(&http_rsp);
        state.mark_clt_send_start();
        clt_send_response
            .send_response(final_rsp, true)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }

        Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
    }

    pub(super) async fn handle_icap_http_response_with_body_after_transfer<CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        mut icap_rsp: RespmodResponse,
        http_header_size: usize,
        orig_http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let mut http_rsp =
            HttpAdaptedResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        let trailers = icap_rsp.take_trailers();
        let has_trailer = !trailers.is_empty();
        http_rsp.set_trailer(trailers);

        let final_rsp = orig_http_response.adapt_to(&http_rsp);
        state.mark_clt_send_start();
        let mut clt_send_stream = clt_send_response
            .send_response(final_rsp, false)
            .map_err(H2RespmodAdaptationError::HttpClientSendHeadFailed)?;
        state.mark_clt_send_header();

        let mut body_transfer = H2StreamFromChunkedTransfer::new(
            &mut self.icap_connection.1,
            &mut clt_send_stream,
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

                r = &mut body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            if icap_rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(RespmodAdaptationEndState::AdaptedTransferred(http_rsp))
                        }
                        Err(H2StreamFromChunkedTransferError::ReadError(e)) => Err(H2RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendDataFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendDataFailed(e)),
                        Err(H2StreamFromChunkedTransferError::SendTrailerFailed(e)) => Err(H2RespmodAdaptationError::HttpClientSendTrailerFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

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
