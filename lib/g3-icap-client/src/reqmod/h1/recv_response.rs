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
    H1ReqmodAdaptationError, HttpAdapterErrorResponse, HttpRequestAdapter,
    HttpRequestForAdaptation, ReqmodAdaptationEndState, ReqmodRecvHttpResponseBody,
};
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
            self.icap_client.save_connection(self.icap_connection).await;
        }
        // there should be a payload
        Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
            icap_rsp.code,
            icap_rsp.reason.to_string(),
        ))
    }

    pub(super) async fn handle_icap_http_response_with_body<H>(
        mut self,
        mut icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
    {
        let mut http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        http_rsp.set_chunked_encoding();
        let trailers = icap_rsp.take_trailers();
        let has_trailer = if trailers.is_empty() {
            false
        } else {
            http_rsp.set_trailer(trailers);
            true
        };
        let recv_body = ReqmodRecvHttpResponseBody {
            icap_client: self.icap_client,
            icap_keepalive: icap_rsp.keep_alive,
            icap_connection: self.icap_connection,
            has_trailer,
        };
        Ok(ReqmodAdaptationEndState::HttpErrResponse(
            http_rsp,
            Some(recv_body),
        ))
    }

    pub(super) async fn handle_icap_http_response_without_body<H>(
        mut self,
        icap_rsp: ReqmodResponse,
        http_header_size: usize,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
    {
        let http_rsp =
            HttpAdapterErrorResponse::parse(&mut self.icap_connection.1, http_header_size).await?;
        if icap_rsp.keep_alive {
            self.icap_client.save_connection(self.icap_connection).await;
        }
        Ok(ReqmodAdaptationEndState::HttpErrResponse(http_rsp, None))
    }
}
