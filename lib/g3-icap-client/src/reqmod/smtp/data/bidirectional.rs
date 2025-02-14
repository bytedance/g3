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

use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncWrite, BufWriter};

use g3_http::server::HttpAdaptedRequest;
use g3_http::{HttpBodyDecodeReader, StreamToChunkedTransfer};
use g3_io_ext::{IdleCheck, LimitedBufReadExt, LimitedCopyConfig, LimitedCopyError};
use g3_smtp_proto::io::TextDataEncodeTransfer;

use super::SmtpAdaptationError;
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::reqmod::response::ReqmodResponse;
use crate::{IcapClientReader, IcapClientWriter, IcapServiceClient};

pub(super) struct BidirectionalRecvIcapResponse<'a, I: IdleCheck> {
    pub(super) icap_client: &'a Arc<IcapServiceClient>,
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) idle_checker: &'a I,
}

impl<I: IdleCheck> BidirectionalRecvIcapResponse<'_, I> {
    pub(super) async fn transfer_and_recv<CR>(
        self,
        mut msg_transfer: &mut StreamToChunkedTransfer<'_, CR, BufWriter<&'_ mut IcapClientWriter>>,
    ) -> Result<ReqmodResponse, SmtpAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
    {
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut msg_transfer => {
                    return match r {
                        Ok(_) => self.recv_icap_response().await,
                        Err(LimitedCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::SmtpClientReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(SmtpAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(SmtpAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if msg_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if msg_transfer.no_cached_data() {
                                Err(SmtpAdaptationError::SmtpClientReadIdle)
                            } else {
                                Err(SmtpAdaptationError::IcapServerWriteIdle)
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

    pub(super) async fn recv_icap_response(self) -> Result<ReqmodResponse, SmtpAdaptationError> {
        let rsp = ReqmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(SmtpAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(SmtpAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpRequest<'a, I: IdleCheck> {
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) idle_checker: &'a I,
    pub(super) http_header_size: usize,
    pub(super) icap_read_finished: bool,
}

impl<I: IdleCheck> BidirectionalRecvHttpRequest<'_, I> {
    pub(super) async fn transfer<CR, UW>(
        &mut self,
        state: &mut ReqmodAdaptationRunState,
        mut clt_msg_transfer: &mut StreamToChunkedTransfer<
            '_,
            CR,
            BufWriter<&'_ mut IcapClientWriter>,
        >,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let _http_req =
            HttpAdaptedRequest::parse(self.icap_reader, self.http_header_size, true).await?;
        // TODO check request content type?

        let mut ups_body_reader = HttpBodyDecodeReader::new_chunked(self.icap_reader, 256);
        let mut ups_buf_writer = BufWriter::new(ups_writer);
        let mut ups_msg_transfer = TextDataEncodeTransfer::new(
            &mut ups_body_reader,
            &mut ups_buf_writer,
            self.copy_config,
        );

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut clt_msg_transfer => {
                    return match r {
                        Ok(_) => {
                            match ups_msg_transfer.await {
                                Ok(_) => {
                                    state.mark_ups_send_all();
                                    if ups_body_reader.trailer(128).await.is_ok() {
                                        self.icap_read_finished = true;
                                    }
                                    Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                                }
                                Err(LimitedCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::IcapServerReadFailed(e)),
                                Err(LimitedCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::SmtpUpstreamWriteFailed(e)),
                            }
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::SmtpClientReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = &mut ups_msg_transfer => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_send_all();
                            if ups_body_reader.trailer(128).await.is_ok() {
                                self.icap_read_finished = true;
                            }
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(SmtpAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(SmtpAdaptationError::SmtpUpstreamWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if clt_msg_transfer.is_idle() && ups_msg_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if clt_msg_transfer.is_idle() {
                                if clt_msg_transfer.no_cached_data() {
                                    Err(SmtpAdaptationError::SmtpClientReadIdle)
                                } else {
                                    Err(SmtpAdaptationError::IcapServerWriteIdle)
                                }
                            } else if ups_msg_transfer.no_cached_data() {
                                Err(SmtpAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(SmtpAdaptationError::SmtpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_msg_transfer.reset_active();
                        ups_msg_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(SmtpAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
