/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_io_ext::IdleCheck;

use super::{
    H2ReqmodAdaptationError, H2RequestAdapter, HttpAdapterErrorResponse, ReqmodAdaptationEndState,
    ReqmodRecvHttpResponseBody,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload(
        self,
        icap_rsp: ReqmodResponse,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection);
        }
        // there should be a payload
        Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
            IcapErrorReason::NoBodyFound,
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_response_with_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<(HttpAdapterErrorResponse, ReqmodRecvHttpResponseBody), H2ReqmodAdaptationError>
    {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.reader, http_header_size)
                .await?;
        let recv_body = ReqmodRecvHttpResponseBody {
            icap_client: self.icap_client,
            icap_keepalive: icap_rsp.keep_alive,
            icap_connection: self.icap_connection,
            copy_config: self.copy_config,
            http_body_line_max_size: self.http_body_line_max_size,
            http_trailer_max_size: self.http_trailer_max_size,
        };
        Ok((http_rsp, recv_body))
    }

    pub(super) async fn handle_icap_http_response_without_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<HttpAdapterErrorResponse, H2ReqmodAdaptationError> {
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
