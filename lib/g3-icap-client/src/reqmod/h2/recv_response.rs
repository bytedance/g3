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

use g3_io_ext::IdleCheck;

use super::{
    H2ReqmodAdaptationError, H2RequestAdapter, HttpAdapterErrorResponse, ReqmodAdaptationEndState,
    ReqmodRecvHttpResponseBody,
};
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    pub(super) async fn handle_icap_ok_without_payload(
        self,
        icap_rsp: ReqmodResponse,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        // there should be a payload
        Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_response_with_body(
        mut self,
        mut icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let mut http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        let trailers = icap_rsp.take_trailers();
        let has_trailer = !trailers.is_empty();
        http_rsp.set_trailer(trailers);
        let recv_body = ReqmodRecvHttpResponseBody {
            icap_client: self.icap_client,
            icap_keepalive: icap_rsp.keep_alive,
            icap_connection: self.icap_connection,
            copy_config: self.copy_config,
            http_body_line_max_size: self.http_body_line_max_size,
            http_trailer_max_size: self.http_trailer_max_size,
            has_trailer,
        };
        Ok(ReqmodAdaptationEndState::HttpErrResponse(
            http_rsp,
            Some(recv_body),
        ))
    }

    pub(super) async fn handle_icap_http_response_without_body(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::HttpErrResponse(http_rsp, None))
    }
}
