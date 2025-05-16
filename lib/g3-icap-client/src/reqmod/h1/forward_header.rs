/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::AsyncWriteExt;

use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    H1ReqmodAdaptationError, HttpRequestAdapter, HttpRequestForAdaptation,
    HttpRequestUpstreamWriter, ReqmodAdaptationEndState, ReqmodAdaptationMidState,
    ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::response::ReqmodResponse;
use crate::reqmod::{IcapReqmodParseError, IcapReqmodResponsePayload};

impl<I: IdleCheck> HttpRequestAdapter<I> {
    fn build_header_only_request<H>(&self, http_request: &H, http_header_len: usize) -> Vec<u8>
    where
        H: HttpRequestForAdaptation,
    {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 64);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        if self.icap_options.support_204 {
            header.put_slice(b"Allow: 204\r\n");
        }
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, null-body={http_header_len}\r\n",
        );
        http_request.append_upgrade_header(&mut header);
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_without_body<H, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: &H,
        ups_writer: &mut UW,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_header_only_request(http_request, http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        self.icap_connection.mark_writer_finished();

        let mut rsp = ReqmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
        }

        match rsp.code {
            204 => {
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                }
                self.handle_original_http_request_without_body(state, rsp, http_request, ups_writer)
                    .await
            }
            n if (200..300).contains(&n) => match rsp.payload {
                IcapReqmodResponsePayload::NoPayload => {
                    self.icap_connection.mark_reader_finished();
                    self.handle_icap_ok_without_payload(rsp).await
                }
                IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                    self.handle_icap_http_request_without_body(
                        state,
                        rsp,
                        header_size,
                        http_request,
                        ups_writer,
                    )
                    .await
                }
                IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        http_request,
                        ups_writer,
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
                Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponse,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }

    pub async fn xfer_connect<H>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: &H,
    ) -> Result<ReqmodAdaptationMidState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
    {
        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_header_only_request(http_request, http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        self.icap_connection.mark_writer_finished();

        let mut rsp = ReqmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
        }

        match rsp.code {
            204 => {
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Ok(ReqmodAdaptationMidState::OriginalRequest)
            }
            n if (200..300).contains(&n) => match rsp.payload {
                IcapReqmodResponsePayload::NoPayload => {
                    self.icap_connection.mark_reader_finished();
                    let _ = self.handle_icap_ok_without_payload::<H>(rsp).await?;
                    Ok(ReqmodAdaptationMidState::OriginalRequest)
                }
                IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                    self.recv_icap_http_request_without_body(rsp, header_size, http_request)
                        .await
                }
                IcapReqmodResponsePayload::HttpRequestWithBody(_) => {
                    Err(H1ReqmodAdaptationError::InvalidIcapServerResponse(
                        IcapReqmodParseError::UnsupportedBody("no body is expected"),
                    ))
                }
                IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => self
                    .handle_icap_http_response_without_body(rsp, header_size)
                    .await
                    .map(|rsp| ReqmodAdaptationMidState::HttpErrResponse(rsp, None)),
                IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => self
                    .handle_icap_http_response_with_body(rsp, header_size)
                    .await
                    .map(|(rsp, body)| ReqmodAdaptationMidState::HttpErrResponse(rsp, Some(body))),
            },
            _ => {
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponse,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }
}
