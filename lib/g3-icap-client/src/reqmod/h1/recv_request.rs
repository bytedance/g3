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
    H1ReqmodAdaptationError, HttpAdaptedRequest, HttpRequestAdapter, HttpRequestForAdaptation,
    HttpRequestUpstreamWriter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reqmod::response::ReqmodResponse;
use crate::reqmod::IcapReqmodResponsePayload;

impl<I: IdleCheck> HttpRequestAdapter<I> {
    pub(super) async fn handle_original_http_request_without_body<H, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_request: &H,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        ups_writer
            .send_request_header(http_request)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        ups_writer
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();
        if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::OriginalTransferred)
    }

    pub(super) async fn handle_original_http_request_with_body<H, CR, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_request: &H,
        clt_body_io: &mut CR,
        clt_body_type: HttpBodyType,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        CR: AsyncBufRead + Unpin,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        ups_writer
            .send_request_header(http_request)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        state.mark_ups_send_header();
        let mut clt_body_reader =
            HttpBodyReader::new(clt_body_io, clt_body_type, self.http_body_line_max_size);
        let mut body_copy = LimitedCopy::new(&mut clt_body_reader, ups_writer, &self.copy_config);

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    match r {
                        Ok(_) => break,
                        Err(LimitedCopyError::ReadFailed(e)) => return Err(H1ReqmodAdaptationError::HttpClientReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => return Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                    }
                }
                _ = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        state.mark_ups_send_all();
        state.clt_read_finished = true;

        if icap_rsp.keep_alive && icap_rsp.payload == IcapReqmodResponsePayload::NoPayload {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::OriginalTransferred)
    }

    pub(super) async fn handle_icap_http_request_without_body<H, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: &H,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.1,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;

        let final_req = orig_http_request.adapt_to(http_req);
        ups_writer
            .send_request_header(&final_req)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        ups_writer
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        state.mark_ups_send_header();
        state.mark_ups_send_no_body();

        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::AdaptedTransferred(final_req))
    }

    pub(super) async fn handle_icap_http_request_with_body_after_transfer<H, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        mut icap_rsp: ReqmodResponse,
        http_header_size: usize,
        orig_http_request: &H,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let mut http_req = HttpAdaptedRequest::parse(
            &mut self.icap_connection.1,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        http_req.set_chunked_encoding();
        let trailers = icap_rsp.take_trailers();
        let body_type = if !trailers.is_empty() {
            http_req.set_trailer(trailers);
            HttpBodyType::ChunkedWithTrailer
        } else {
            HttpBodyType::ChunkedWithoutTrailer
        };

        let final_req = orig_http_request.adapt_to(http_req);
        ups_writer
            .send_request_header(&final_req)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        state.mark_ups_send_header();

        let mut body_reader = HttpBodyReader::new(
            &mut self.icap_connection.1,
            body_type,
            self.http_body_line_max_size,
        );
        let mut body_copy = LimitedCopy::new(&mut body_reader, ups_writer, &self.copy_config);

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    match r {
                        Ok(_) => break,
                        Err(LimitedCopyError::ReadFailed(e)) => return Err(H1ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => return Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                    }
                }
                _ = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_copy.no_cached_data() {
                                Err(H1ReqmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_copy.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }

        state.mark_ups_send_all();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::AdaptedTransferred(final_req))
    }
}
