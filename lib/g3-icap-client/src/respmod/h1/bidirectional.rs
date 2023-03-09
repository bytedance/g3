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

use tokio::io::AsyncBufRead;
use tokio::time::Instant;

use g3_http::{ChunkedTransfer, HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedBufReadExt, LimitedCopy, LimitedCopyConfig, LimitedCopyError};

use super::{
    H1RespmodAdaptationError, HttpAdaptedResponse, HttpResponseClientWriter,
    HttpResponseForAdaptation, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::respmod::response::RespmodResponse;
use crate::{IcapClientReader, IcapClientWriter, IcapServiceClient};

pub(super) struct BidirectionalRecvIcapResponse<'a, I: IdleCheck> {
    pub(super) icap_client: &'a Arc<IcapServiceClient>,
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) idle_checker: &'a I,
}

impl<'a, I: IdleCheck> BidirectionalRecvIcapResponse<'a, I> {
    pub(super) async fn transfer_and_recv<UR>(
        self,
        mut body_transfer: &mut ChunkedTransfer<'_, UR, IcapClientWriter>,
    ) -> Result<RespmodResponse, H1RespmodAdaptationError>
    where
        UR: AsyncBufRead + Unpin,
    {
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
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(H1RespmodAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(H1RespmodAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H1RespmodAdaptationError::HttpUpstreamReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::IcapServerWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn recv_icap_response(
        self,
    ) -> Result<RespmodResponse, H1RespmodAdaptationError> {
        let rsp = RespmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpResponse<'a, I: IdleCheck> {
    pub(super) icap_rsp: RespmodResponse,
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) http_body_line_max_size: usize,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) idle_checker: &'a I,
}

impl<'a, I: IdleCheck> BidirectionalRecvHttpResponse<'a, I> {
    pub(super) async fn transfer<H, UR, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        mut ups_body_transfer: &mut ChunkedTransfer<'_, UR, IcapClientWriter>,
        http_header_size: usize,
        orig_http_response: &H,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let mut http_rsp = HttpAdaptedResponse::parse(self.icap_reader, http_header_size).await?;
        http_rsp.set_chunked_encoding();
        let trailers = self.icap_rsp.take_trailers();
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

        let mut adp_body_reader =
            HttpBodyReader::new(self.icap_reader, body_type, self.http_body_line_max_size);
        let mut adp_body_transfer =
            LimitedCopy::new(&mut adp_body_reader, clt_writer, &self.copy_config);

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut ups_body_transfer => {
                    return match r {
                        Ok(_) => {
                            match adp_body_transfer.await {
                                Ok(_) => {
                                    state.mark_clt_send_all();
                                    state.icap_io_finished = true;
                                    Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
                                }
                                Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::IcapServerReadFailed(e)),
                                Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                            }
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = &mut adp_body_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_clt_send_all();
                            if ups_body_transfer.finished() {
                                state.icap_io_finished = true;
                            }
                            Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if ups_body_transfer.is_idle() && adp_body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if ups_body_transfer.is_idle() {
                                if ups_body_transfer.no_cached_data() {
                                    Err(H1RespmodAdaptationError::HttpUpstreamReadIdle)
                                } else {
                                    Err(H1RespmodAdaptationError::IcapServerWriteIdle)
                                }
                            } else if adp_body_transfer.no_cached_data() {
                                Err(H1RespmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H1RespmodAdaptationError::HttpClientWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        ups_body_transfer.reset_active();
                        adp_body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
