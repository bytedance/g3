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

use anyhow::anyhow;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use tokio::time::Instant;

use g3_http::{H1BodyToChunkedTransfer, HttpBodyDecodeReader, HttpBodyReader};
use g3_io_ext::{IdleCheck, LimitedBufReadExt, LimitedCopy, LimitedCopyConfig, LimitedCopyError};

use super::{
    H1ReqmodAdaptationError, HttpAdaptedRequest, HttpRequestForAdaptation,
    HttpRequestUpstreamWriter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
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
        mut body_transfer: &mut H1BodyToChunkedTransfer<'_, CR, IcapClientWriter>,
    ) -> Result<ReqmodResponse, H1ReqmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
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
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::HttpClientReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(H1ReqmodAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(H1ReqmodAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if body_transfer.no_cached_data() {
                                Err(H1ReqmodAdaptationError::HttpClientReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::IcapServerWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn recv_icap_response(
        self,
    ) -> Result<ReqmodResponse, H1ReqmodAdaptationError> {
        let rsp = ReqmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                IcapErrorReason::InvalidResponseAfterContinue,
                rsp.code,
                rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                IcapErrorReason::UnknownResponseAfterContinue,
                rsp.code,
                rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpRequest<'a, I: IdleCheck> {
    pub(super) http_body_line_max_size: usize,
    pub(super) http_req_add_no_via_header: bool,
    pub(super) copy_config: LimitedCopyConfig,
    pub(super) idle_checker: &'a I,
}

impl<I: IdleCheck> BidirectionalRecvHttpRequest<'_, I> {
    pub(super) async fn transfer<H, CR, UW>(
        self,
        state: &mut ReqmodAdaptationRunState,
        clt_body_transfer: &mut H1BodyToChunkedTransfer<'_, CR, IcapClientWriter>,
        http_header_size: usize,
        orig_http_request: &H,
        icap_reader: &mut IcapClientReader,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        CR: AsyncBufRead + Unpin,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let http_req = HttpAdaptedRequest::parse(
            icap_reader,
            http_header_size,
            self.http_req_add_no_via_header,
        )
        .await?;
        let body_content_length = http_req.content_length;

        let final_req = orig_http_request.adapt_to(http_req);
        ups_writer
            .send_request_header(&final_req)
            .await
            .map_err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed)?;
        state.mark_ups_send_header();

        match body_content_length {
            Some(0) => Err(H1ReqmodAdaptationError::InvalidHttpBodyFromIcapServer(
                anyhow!("Content-Length is 0 but the ICAP server response contains http-body"),
            )),
            Some(expected) => {
                let mut ups_body_reader =
                    HttpBodyDecodeReader::new_chunked(icap_reader, self.http_body_line_max_size);
                let mut ups_body_transfer =
                    LimitedCopy::new(&mut ups_body_reader, ups_writer, &self.copy_config);
                self.do_transfer(clt_body_transfer, &mut ups_body_transfer)
                    .await?;

                state.mark_ups_send_all();
                let copied = ups_body_transfer.copied_size();
                if clt_body_transfer.finished() && ups_body_reader.trailer(128).await.is_ok() {
                    state.icap_io_finished = true;
                }

                if copied != expected {
                    return Err(H1ReqmodAdaptationError::InvalidHttpBodyFromIcapServer(
                        anyhow!("Content-Length is {expected} but decoded length is {copied}"),
                    ));
                }
                Ok(ReqmodAdaptationEndState::AdaptedTransferred(final_req))
            }
            None => {
                let mut ups_body_reader =
                    HttpBodyReader::new_chunked(icap_reader, self.http_body_line_max_size);
                let mut ups_body_transfer =
                    LimitedCopy::new(&mut ups_body_reader, ups_writer, &self.copy_config);
                self.do_transfer(clt_body_transfer, &mut ups_body_transfer)
                    .await?;

                state.mark_ups_send_all();
                state.icap_io_finished =
                    clt_body_transfer.finished() && ups_body_transfer.finished();

                Ok(ReqmodAdaptationEndState::AdaptedTransferred(final_req))
            }
        }
    }

    async fn do_transfer<CR, IR, UW>(
        &self,
        mut clt_body_transfer: &mut H1BodyToChunkedTransfer<'_, CR, IcapClientWriter>,
        mut ups_body_transfer: &mut LimitedCopy<'_, IR, UW>,
    ) -> Result<(), H1ReqmodAdaptationError>
    where
        CR: AsyncBufRead + Unpin,
        IR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let idle_duration = self.idle_checker.idle_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut clt_body_transfer => {
                    return match r {
                        Ok(_) => {
                            match ups_body_transfer.await {
                                Ok(_) => Ok(()),
                                Err(LimitedCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::IcapServerReadFailed(e)),
                                Err(LimitedCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                            }
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::HttpClientReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = &mut ups_body_transfer => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(H1ReqmodAdaptationError::IcapServerReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_body_transfer.is_idle() && ups_body_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if clt_body_transfer.is_idle() {
                                if clt_body_transfer.no_cached_data() {
                                    Err(H1ReqmodAdaptationError::HttpClientReadIdle)
                                } else {
                                    Err(H1ReqmodAdaptationError::IcapServerWriteIdle)
                                }
                            } else if ups_body_transfer.no_cached_data() {
                                Err(H1ReqmodAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(H1ReqmodAdaptationError::HttpUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_body_transfer.reset_active();
                        ups_body_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(H1ReqmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
