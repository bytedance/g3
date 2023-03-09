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

use std::io::Write;

use bytes::BufMut;
use http::{Request, Response};
use tokio::io::AsyncWriteExt;

use g3_h2::{RequestExt, ResponseExt};
use g3_io_ext::IdleCheck;

use super::{
    H2RespmodAdaptationError, H2ResponseAdapter, H2SendResponseToClient, RespmodAdaptationEndState,
    RespmodAdaptationRunState,
};
use crate::respmod::response::RespmodResponse;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> H2ResponseAdapter<I> {
    fn build_header_only_request(
        &self,
        http_req_hdr_len: usize,
        http_rsp_hdr_len: usize,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 64);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        if self.icap_options.support_204 {
            header.put_slice(b"Allow: 204\r\n");
        }
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, null-body={}\r\n",
            http_req_hdr_len + http_rsp_hdr_len
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_without_body<CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        http_request: &Request<()>,
        http_response: Response<()>,
        clt_send_response: &mut CW,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();
        let icap_header =
            self.build_header_only_request(http_req_header.len(), http_rsp_header.len());

        let icap_w = &mut self.icap_connection.0;
        icap_w
            .write_all(&icap_header)
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_req_header)
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_rsp_header)
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;

        let rsp = RespmodResponse::parse(
            &mut self.icap_connection.1,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            204 => {
                self.handle_original_http_response_without_body(
                    state,
                    rsp,
                    http_response,
                    clt_send_response,
                )
                .await
            }
            n if (200..300).contains(&n) => match rsp.payload {
                IcapRespmodResponsePayload::NoPayload => {
                    self.handle_icap_ok_without_payload(rsp).await
                }
                IcapRespmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                    self.handle_icap_http_response_without_body(
                        state,
                        rsp,
                        header_size,
                        http_response,
                        clt_send_response,
                    )
                    .await
                }
                IcapRespmodResponsePayload::HttpResponseWithBody(header_size) => {
                    self.handle_icap_http_response_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        http_response,
                        clt_send_response,
                    )
                    .await
                }
            },
            _ => {
                if rsp.keep_alive && rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_client.save_connection(self.icap_connection).await;
                }
                Err(H2RespmodAdaptationError::IcapServerErrorResponse(
                    rsp.code, rsp.reason,
                ))
            }
        }
    }
}
