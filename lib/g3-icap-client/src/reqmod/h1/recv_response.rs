/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_io_ext::IdleCheck;

use super::{
    H1ReqmodAdaptationError, HttpAdapterErrorResponse, HttpRequestAdapter,
    HttpRequestForAdaptation, ReqmodAdaptationEndState, ReqmodRecvHttpResponseBody,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> HttpRequestAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload<H>(
        self,
        icap_rsp: ReqmodResponse,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
    {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a payload
        Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
            IcapErrorReason::NoBodyFound,
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_response_with_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<(HttpAdapterErrorResponse, ReqmodRecvHttpResponseBody), H1ReqmodAdaptationError>
    {
        let mut http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.reader, http_header_size)
                .await?;
        http_rsp.set_chunked_encoding();
        let recv_body = ReqmodRecvHttpResponseBody {
            icap_client: self.icap_client,
            icap_keepalive: icap_rsp.keep_alive,
            icap_connection: self.icap_connection,
        };
        Ok((http_rsp, recv_body))
    }

    pub(super) async fn handle_icap_http_response_without_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<HttpAdapterErrorResponse, H1ReqmodAdaptationError> {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.reader, http_header_size)
                .await?;

        self.icap_connection.mark_reader_finished();
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        Ok(http_rsp)
    }
}
