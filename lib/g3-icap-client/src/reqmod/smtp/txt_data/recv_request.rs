/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use tokio::io::AsyncWrite;
use tokio::time::Instant;

use g3_http::server::HttpAdaptedRequest;
use g3_http::HttpBodyDecodeReader;
use g3_io_ext::{IdleCheck, LimitedCopy, LimitedCopyError};

use super::{
    ReqmodAdaptationEndState, ReqmodAdaptationRunState, SmtpAdaptationError, SmtpMessageAdapter,
};
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> SmtpMessageAdapter<I> {
    pub(super) async fn handle_icap_http_request_without_body(
        mut self,
        _state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError> {
        let _http_req =
            HttpAdaptedRequest::parse(&mut self.icap_connection.1, http_header_size, true).await?;

        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        // there should be a message body
        Err(SmtpAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_request_with_body_after_transfer<UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError>
    where
        UW: AsyncWrite + Unpin,
    {
        let _http_req =
            HttpAdaptedRequest::parse(&mut self.icap_connection.1, http_header_size, true).await?;
        // TODO check request content type?

        let mut body_reader = HttpBodyDecodeReader::new_chunked(&mut self.icap_connection.1, 256);
        let mut msg_transfer = LimitedCopy::new(&mut body_reader, ups_writer, &self.copy_config);
        // TODO encode to TEXT DATA

        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut msg_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            if icap_rsp.keep_alive && body_reader.trailer(128).await.is_ok() {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                        },
                        Err(LimitedCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::SmtpUpstreamWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if msg_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if msg_transfer.no_cached_data() {
                                Err(SmtpAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(SmtpAdaptationError::SmtpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        msg_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(SmtpAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
