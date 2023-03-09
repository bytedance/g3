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

use tokio::io::{AsyncBufRead, AsyncWriteExt};
use tokio::time::Instant;

use g3_http::{HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedCopy, LimitedCopyError};

use super::{
    H1RespmodAdaptationError, HttpAdaptedResponse, HttpResponseAdapter, HttpResponseClientWriter,
    HttpResponseForAdaptation, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::respmod::response::RespmodResponse;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> HttpResponseAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload<H>(
        self,
        icap_rsp: RespmodResponse,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        // there should be a payload
        Err(H1RespmodAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_original_http_response_without_body<H, CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_response: &H,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        state.mark_clt_send_start();
        clt_writer
            .send_response_header(http_response)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        clt_writer
            .flush()
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();
        if icap_rsp.keep_alive && icap_rsp.payload == IcapRespmodResponsePayload::NoPayload {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(RespmodAdaptationEndState::OriginalTransferred)
    }

    pub(super) async fn handle_original_http_response_with_body<H, UR, CW>(
        self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_response: &H,
        ups_body_io: &mut UR,
        ups_body_type: HttpBodyType,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        state.mark_clt_send_start();
        clt_writer
            .send_response_header(http_response)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();

        let mut ups_body_reader =
            HttpBodyReader::new(ups_body_io, ups_body_type, self.http_body_line_max_size);
        let mut body_copy = LimitedCopy::new(&mut ups_body_reader, clt_writer, &self.copy_config);

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_recv_all();
                            state.mark_clt_send_all();
                            if icap_rsp.keep_alive && icap_rsp.payload == IcapRespmodResponsePayload::NoPayload {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(RespmodAdaptationEndState::OriginalTransferred)
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1RespmodAdaptationError::HttpUpstreamReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn handle_icap_http_response_without_body<H, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        icap_rsp: RespmodResponse,
        http_header_size: usize,
        orig_http_response: &H,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let http_rsp =
            HttpAdaptedResponse::parse(&mut self.icap_connection.1, http_header_size).await?;

        let final_rsp = orig_http_response.adapt_to(http_rsp);
        state.mark_clt_send_start();
        clt_writer
            .send_response_header(&final_rsp)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        clt_writer
            .flush()
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();
        state.mark_clt_send_no_body();

        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
    }

    pub(super) async fn handle_icap_http_response_with_body_after_transfer<H, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        mut icap_rsp: RespmodResponse,
        http_header_size: usize,
        orig_http_response: &H,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let mut http_rsp =
            HttpAdaptedResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        http_rsp.set_chunked_encoding();
        let trailers = icap_rsp.take_trailers();
        let body_type = if !trailers.is_empty() {
            http_rsp.set_trailer(trailers);
            HttpBodyType::ChunkedWithTrailer
        } else {
            HttpBodyType::ChunkedWithoutTrailer
        };

        let final_rsp = orig_http_response.adapt_to(http_rsp);
        state.mark_clt_send_start();
        clt_writer
            .send_response_header(&final_rsp)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();

        let mut body_reader = HttpBodyReader::new(
            &mut self.icap_connection.1,
            body_type,
            self.http_body_line_max_size,
        );
        let mut body_copy = LimitedCopy::new(&mut body_reader, clt_writer, &self.copy_config);

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            if icap_rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1RespmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
