/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};

use g3_io_ext::{IdleCheck, LimitedCopy, LimitedWriteExt};

use super::{
    HttpAdapterErrorResponse, ImapAdaptationError, ImapMessageAdapter, ReqmodAdaptationEndState,
    ReqmodAdaptationRunState, ReqmodRecvHttpResponseBody,
};
use crate::reqmod::IcapReqmodResponsePayload;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod recv_request;
mod recv_response;

impl<I: IdleCheck> ImapMessageAdapter<I> {
    fn build_forward_all_request(&self, http_header_len: usize, literal_size: u64) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 64);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\n",
        );
        header.put_slice(b"\r\n");
        let _ = write!(header, "{literal_size:x}\r\n");
        header
    }

    pub async fn xfer_append_without_preview<CR, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        clt_r: &mut CR,
        literal_size: u64,
        ups_w: &mut UW,
    ) -> Result<ReqmodAdaptationEndState, ImapAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let http_header = self.build_http_header(literal_size);
        let icap_header = self.build_forward_all_request(http_header.len(), literal_size);

        let icap_w = &mut self.icap_connection.0;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(ImapAdaptationError::IcapServerWriteFailed)?;

        let mut message_reader =
            BufReader::with_capacity(self.copy_config.buffer_size(), clt_r.take(literal_size));
        let mut body_transfer = LimitedCopy::new(
            &mut message_reader,
            &mut self.icap_connection.0,
            &self.copy_config,
        );

        let bidirectional_transfer = BidirectionalRecvIcapResponse {
            icap_client: &self.icap_client,
            icap_reader: &mut self.icap_connection.1,
            idle_checker: &self.idle_checker,
        };
        let rsp = bidirectional_transfer
            .transfer_and_recv(&mut body_transfer)
            .await?;
        if body_transfer.finished() {
            state.clt_read_finished = true;
        }

        match rsp.payload {
            IcapReqmodResponsePayload::NoPayload => self.handle_icap_ok_without_payload(rsp).await,
            IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
                self.handle_icap_http_request_without_body(state, rsp, header_size)
                    .await
            }
            IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                if body_transfer.finished() {
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        ups_w,
                    )
                    .await
                } else {
                    let icap_keepalive = rsp.keep_alive;
                    let bidirectional_transfer = BidirectionalRecvHttpRequest {
                        icap_reader: &mut self.icap_connection.1,
                        copy_config: self.copy_config,
                        idle_checker: &self.idle_checker,
                    };
                    let r = bidirectional_transfer
                        .transfer(state, &mut body_transfer, header_size, ups_w)
                        .await?;
                    if message_reader.get_ref().limit() == 0 {
                        state.clt_read_finished = true;
                    }
                    if icap_keepalive && state.icap_io_finished {
                        self.icap_client.save_connection(self.icap_connection).await;
                    }
                    Ok(r)
                }
            }
            IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => self
                .handle_icap_http_response_without_body(rsp, header_size)
                .await
                .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None)),
            IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => self
                .handle_icap_http_response_with_body(rsp, header_size)
                .await
                .map(|(rsp, body)| ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))),
        }
    }
}
