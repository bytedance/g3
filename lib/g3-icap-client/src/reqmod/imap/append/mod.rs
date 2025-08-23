/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_io_ext::{IdleCheck, LimitedWriteExt, StreamCopy};

use super::{HttpAdapterErrorResponse, ImapAdaptationError, ImapMessageAdapter};
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::reqmod::response::ReqmodResponse;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod recv_request;
mod recv_response;

impl<I: IdleCheck> ImapMessageAdapter<I> {
    fn build_forward_all_request(&self, http_header_len: usize, all_cached: bool) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 64);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        if self.icap_options.support_204 && all_cached {
            header.put_slice(b"Allow: 204\r\n");
        }
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    fn build_chunked_header(&self) -> String {
        format!("{:x}\r\n", self.literal_size)
    }

    pub async fn xfer_append_once<UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        cached: &[u8],
        ups_w: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        UW: AsyncWrite + Unpin,
    {
        let http_header = self.build_http_header();
        let icap_header = self.build_forward_all_request(http_header.len(), true);
        // TODO support 204
        let chunked_header = self.build_chunked_header();

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_header),
                IoSlice::new(chunked_header.as_bytes()),
                IoSlice::new(cached),
                IoSlice::new(b"\r\n0\r\n\r\n"),
            ])
            .await
            .map_err(ImapAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(ImapAdaptationError::IcapServerWriteFailed)?;
        self.icap_connection.mark_writer_finished();

        let rsp = ReqmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;

        match rsp.code {
            204 => {
                self.icap_connection.mark_reader_finished();
                if rsp.keep_alive {
                    self.icap_client.save_connection(self.icap_connection);
                }

                ups_w
                    .write_all(cached)
                    .await
                    .map_err(ImapAdaptationError::ImapUpstreamWriteFailed)?;
                Ok(ReqmodAdaptationEndState::OriginalTransferred)
            }
            206 => Err(ImapAdaptationError::IcapServerErrorResponse(
                rsp.code, rsp.reason,
            )),
            n if (200..300).contains(&n) => match rsp.payload {
                IcapReqmodResponsePayload::NoPayload => {
                    self.icap_connection.mark_reader_finished();
                    self.handle_icap_ok_without_payload(rsp).await
                }
                IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                    self.handle_icap_http_request_without_body(state, rsp, header_size)
                        .await
                }
                IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        ups_w,
                    )
                    .await
                }
                IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => self
                    .handle_icap_http_response_without_body(rsp, header_size)
                    .await
                    .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None)),
                IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => self
                    .handle_icap_http_response_with_body(rsp, header_size)
                    .await
                    .map(|(rsp, body)| ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))),
            },
            _ => {
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(ImapAdaptationError::IcapServerErrorResponse(
                    rsp.code, rsp.reason,
                ))
            }
        }
    }

    pub async fn xfer_append_without_preview<CR, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        clt_r: &mut CR,
        cached: &[u8],
        read_size: u64,
        ups_w: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let http_header = self.build_http_header();
        let icap_header = self.build_forward_all_request(http_header.len(), false);
        let chunked_header = self.build_chunked_header();

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_header),
                IoSlice::new(chunked_header.as_bytes()),
                IoSlice::new(cached),
            ])
            .await
            .map_err(ImapAdaptationError::IcapServerWriteFailed)?;

        let mut message_reader = clt_r.take(read_size);
        let mut body_transfer = StreamCopy::new(
            &mut message_reader,
            &mut self.icap_connection.writer,
            &self.copy_config,
        );

        let bidirectional_transfer = BidirectionalRecvIcapResponse {
            icap_client: &self.icap_client,
            icap_reader: &mut self.icap_connection.reader,
            idle_checker: &self.idle_checker,
        };
        let rsp = bidirectional_transfer
            .transfer_and_recv(&mut body_transfer)
            .await?;
        if body_transfer.finished() {
            state.clt_read_finished = true;
        }

        match rsp.payload {
            IcapReqmodResponsePayload::NoPayload => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
                self.icap_connection.mark_reader_finished();
                self.handle_icap_ok_without_payload(rsp).await
            }
            IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
                self.handle_icap_http_request_without_body(state, rsp, header_size)
                    .await
            }
            IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        ups_w,
                    )
                    .await
                } else {
                    let mut bidirectional_transfer = BidirectionalRecvHttpRequest {
                        icap_reader: &mut self.icap_connection.reader,
                        copy_config: self.copy_config,
                        idle_checker: &self.idle_checker,
                        http_header_size: header_size,
                        imap_message_size: self.literal_size,
                        icap_read_finished: false,
                    };
                    let r = bidirectional_transfer
                        .transfer(state, &mut body_transfer, ups_w)
                        .await?;
                    let icap_read_finished = bidirectional_transfer.icap_read_finished;
                    if body_transfer.finished() {
                        if message_reader.limit() == 0 {
                            state.clt_read_finished = true;
                        }
                        self.icap_connection.mark_writer_finished();
                        if icap_read_finished {
                            self.icap_connection.mark_reader_finished();
                            if rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection);
                            }
                        }
                    }
                    Ok(r)
                }
            }
            IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
                self.handle_icap_http_response_without_body(rsp, header_size)
                    .await
                    .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None))
            }
            IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
                self.handle_icap_http_response_with_body(rsp, header_size)
                    .await
                    .map(|(rsp, body)| ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body)))
            }
        }
    }
}
