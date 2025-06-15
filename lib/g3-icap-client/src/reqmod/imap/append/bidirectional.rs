/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite, BufWriter};

use g3_http::HttpBodyDecodeReader;
use g3_http::server::HttpAdaptedRequest;
use g3_io_ext::{
    IdleCheck, LimitedBufReadExt, LimitedWriteExt, StreamCopy, StreamCopyConfig, StreamCopyError,
};

use super::ImapAdaptationError;
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
        mut msg_transfer: &mut StreamCopy<'_, CR, IcapClientWriter>,
    ) -> Result<ReqmodResponse, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
    {
        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut msg_transfer => {
                    return match r {
                        Ok(_) => {
                            msg_transfer
                                .writer()
                                .write_all_flush(b"\r\n0\r\n\r\n")
                                .await
                                .map_err(ImapAdaptationError::IcapServerWriteFailed)?;
                            self.recv_icap_response().await
                        }
                        Err(StreamCopyError::ReadFailed(e)) => Err(ImapAdaptationError::ImapClientReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(ImapAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = self.icap_reader.fill_wait_data() => {
                    return match r {
                        Ok(true) => self.recv_icap_response().await,
                        Ok(false) => Err(ImapAdaptationError::IcapServerConnectionClosed),
                        Err(e) => Err(ImapAdaptationError::IcapServerReadFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if msg_transfer.is_idle() {
                        idle_count += n;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if msg_transfer.no_cached_data() {
                                Err(ImapAdaptationError::ImapClientReadIdle)
                            } else {
                                Err(ImapAdaptationError::IcapServerWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        msg_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(ImapAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }

    pub(super) async fn recv_icap_response(self) -> Result<ReqmodResponse, ImapAdaptationError> {
        let rsp = ReqmodResponse::parse(
            self.icap_reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;

        match rsp.code {
            204 | 206 => Err(ImapAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
            n if (200..300).contains(&n) => Ok(rsp),
            _ => Err(ImapAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
        }
    }
}

pub(super) struct BidirectionalRecvHttpRequest<'a, I: IdleCheck> {
    pub(super) icap_reader: &'a mut IcapClientReader,
    pub(super) copy_config: StreamCopyConfig,
    pub(super) idle_checker: &'a I,
    pub(super) http_header_size: usize,
    pub(super) imap_message_size: u64,
    pub(super) icap_read_finished: bool,
}

impl<I: IdleCheck> BidirectionalRecvHttpRequest<'_, I> {
    pub(super) async fn transfer<CR, UW>(
        &mut self,
        state: &mut ReqmodAdaptationRunState,
        mut clt_msg_transfer: &mut StreamCopy<'_, CR, IcapClientWriter>,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let http_req =
            HttpAdaptedRequest::parse(self.icap_reader, self.http_header_size, true).await?;
        if let Some(len) = http_req.content_length {
            if len != self.imap_message_size {
                return Err(ImapAdaptationError::MessageSizeNotMatch);
            }
        }
        // TODO check request content type?

        let mut ups_body_reader = HttpBodyDecodeReader::new_chunked(self.icap_reader, 256);
        let mut ups_buf_writer = BufWriter::new(ups_writer);
        let mut ups_msg_transfer =
            StreamCopy::new(&mut ups_body_reader, &mut ups_buf_writer, &self.copy_config);

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                r = &mut clt_msg_transfer => {
                    return match r {
                        Ok(_) => {
                            clt_msg_transfer
                                .writer()
                                .write_all_flush(b"\r\n0\r\n\r\n")
                                .await
                                .map_err(ImapAdaptationError::IcapServerWriteFailed)?;
                            match ups_msg_transfer.await {
                                Ok(copied) => {
                                    state.mark_ups_send_all();
                                    if ups_body_reader.trailer(128).await.is_ok() {
                                        self.icap_read_finished = true;
                                    }
                                    if copied != self.imap_message_size {
                                        return Err(ImapAdaptationError::MessageSizeNotMatch);
                                    }
                                    Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                                }
                                Err(StreamCopyError::ReadFailed(e)) => Err(ImapAdaptationError::IcapServerReadFailed(e)),
                                Err(StreamCopyError::WriteFailed(e)) => Err(ImapAdaptationError::ImapUpstreamWriteFailed(e)),
                            }
                        }
                        Err(StreamCopyError::ReadFailed(e)) => Err(ImapAdaptationError::ImapClientReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(ImapAdaptationError::IcapServerWriteFailed(e)),
                    };
                }
                r = &mut ups_msg_transfer => {
                    return match r {
                        Ok(copied) => {
                            state.mark_ups_send_all();
                            if ups_body_reader.trailer(128).await.is_ok() {
                                self.icap_read_finished = true;
                            }
                            if copied != self.imap_message_size {
                                return Err(ImapAdaptationError::MessageSizeNotMatch);
                            }
                            Ok(ReqmodAdaptationEndState::AdaptedTransferred)
                        }
                        Err(StreamCopyError::ReadFailed(e)) => Err(ImapAdaptationError::IcapServerReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(ImapAdaptationError::ImapUpstreamWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_msg_transfer.is_idle() && ups_msg_transfer.is_idle() {
                        idle_count += 1;

                        let quit = self.idle_checker.check_quit(idle_count);
                        if quit {
                            return if clt_msg_transfer.is_idle() {
                                if clt_msg_transfer.no_cached_data() {
                                    Err(ImapAdaptationError::ImapClientReadIdle)
                                } else {
                                    Err(ImapAdaptationError::IcapServerWriteIdle)
                                }
                            } else if ups_msg_transfer.no_cached_data() {
                                Err(ImapAdaptationError::IcapServerReadIdle)
                            } else {
                                Err(ImapAdaptationError::ImapUpstreamWriteIdle)
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_msg_transfer.reset_active();
                        ups_msg_transfer.reset_active();
                    }

                    if let Some(reason) = self.idle_checker.check_force_quit() {
                        return Err(ImapAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
