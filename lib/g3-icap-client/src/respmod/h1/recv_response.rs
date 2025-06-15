/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt};

use g3_http::{HttpBodyDecodeReader, HttpBodyReader, HttpBodyType};
use g3_io_ext::{IdleCheck, StreamCopy, StreamCopyError};

use super::{
    H1RespmodAdaptationError, HttpAdaptedResponse, HttpResponseAdapter, HttpResponseClientWriter,
    HttpResponseForAdaptation, RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::respmod::response::RespmodResponse;

impl<I: IdleCheck> HttpResponseAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload<H>(
        self,
        icap_rsp: RespmodResponse,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        H: HttpResponseForAdaptation,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a payload
        Err(H1RespmodAdaptationError::IcapServerErrorResponse(
            IcapErrorReason::NoBodyFound,
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
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

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
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        state.mark_clt_send_start();
        clt_writer
            .send_response_header(http_response)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();

        let mut ups_body_reader =
            HttpBodyReader::new(ups_body_io, ups_body_type, self.http_body_line_max_size);
        let mut body_copy = StreamCopy::new(&mut ups_body_reader, clt_writer, &self.copy_config);

        let mut idle_interval = self.idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => {
                            state.mark_ups_recv_all();
                            state.mark_clt_send_all();
                            Ok(RespmodAdaptationEndState::OriginalTransferred)
                        }
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::HttpUpstreamReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += n;

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
            HttpAdaptedResponse::parse(&mut self.icap_connection.reader, http_header_size).await?;
        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }

        let final_rsp = orig_http_response.adapt_without_body(http_rsp);
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

        Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
    }

    pub(super) async fn handle_icap_http_response_with_body_after_transfer<H, CW>(
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
            HttpAdaptedResponse::parse(&mut self.icap_connection.reader, http_header_size).await?;
        let body_content_length = http_rsp.content_length;

        let final_rsp = orig_http_response.adapt_with_body(http_rsp);
        state.mark_clt_send_start();
        clt_writer
            .send_response_header(&final_rsp)
            .await
            .map_err(H1RespmodAdaptationError::HttpClientWriteFailed)?;
        state.mark_clt_send_header();

        match body_content_length {
            Some(0) => Err(H1RespmodAdaptationError::InvalidHttpBodyFromIcapServer(
                anyhow!("Content-Length is 0 but the ICAP server response contains http-body"),
            )),
            Some(expected) => {
                let mut body_reader = HttpBodyDecodeReader::new_chunked(
                    &mut self.icap_connection.reader,
                    self.http_body_line_max_size,
                );
                let mut body_copy =
                    StreamCopy::new(&mut body_reader, clt_writer, &self.copy_config);
                Self::send_response_body(&self.idle_checker, &mut body_copy).await?;

                state.mark_clt_send_all();
                let copied = body_copy.copied_size();

                if body_reader.trailer(128).await.is_ok() {
                    self.icap_connection.mark_reader_finished();
                    if icap_rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }

                if copied != expected {
                    return Err(H1RespmodAdaptationError::InvalidHttpBodyFromIcapServer(
                        anyhow!("Content-Length is {expected} but decoded length is {copied}"),
                    ));
                }
                Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
            }
            None => {
                let mut body_reader = HttpBodyReader::new_chunked(
                    &mut self.icap_connection.reader,
                    self.http_body_line_max_size,
                );
                let mut body_copy =
                    StreamCopy::new(&mut body_reader, clt_writer, &self.copy_config);
                Self::send_response_body(&self.idle_checker, &mut body_copy).await?;

                state.mark_clt_send_all();
                self.icap_connection.mark_reader_finished();
                if icap_rsp.keep_alive {
                    self.icap_client.save_connection(self.icap_connection);
                }
                Ok(RespmodAdaptationEndState::AdaptedTransferred(final_rsp))
            }
        }
    }

    async fn send_response_body<R, W>(
        idle_checker: &I,
        mut body_copy: &mut StreamCopy<'_, R, W>,
    ) -> Result<(), H1RespmodAdaptationError>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut idle_interval = idle_checker.interval_timer();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut body_copy => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(StreamCopyError::ReadFailed(e)) => Err(H1RespmodAdaptationError::IcapServerReadFailed(e)),
                        Err(StreamCopyError::WriteFailed(e)) => Err(H1RespmodAdaptationError::HttpClientWriteFailed(e)),
                    };
                }
                n = idle_interval.tick() => {
                    if body_copy.is_idle() {
                        idle_count += n;

                        let quit = idle_checker.check_quit(idle_count);
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

                    if let Some(reason) = idle_checker.check_force_quit() {
                        return Err(H1RespmodAdaptationError::IdleForceQuit(reason));
                    }
                }
            }
        }
    }
}
