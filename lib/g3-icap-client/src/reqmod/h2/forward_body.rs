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

use std::io::{IoSlice, Write};

use bytes::{BufMut, Bytes};
use h2::RecvStream;
use h2::client::SendRequest;
use http::Request;

use g3_h2::{H2StreamToChunkedTransfer, RequestExt};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H2ReqmodAdaptationError,
    H2RequestAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reqmod::IcapReqmodResponsePayload;

impl<I: IdleCheck> H2RequestAdapter<I> {
    fn build_forward_all_request(&self, http_header_len: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header, None);
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_without_preview(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        mut clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_forward_all_request(http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

        let mut body_transfer = H2StreamToChunkedTransfer::new(
            &mut clt_body,
            &mut self.icap_connection.writer,
            self.copy_config.yield_size(),
        );
        let bidirectional_transfer = BidirectionalRecvIcapResponse {
            icap_client: &self.icap_client,
            icap_reader: &mut self.icap_connection.reader,
            idle_checker: &self.idle_checker,
        };
        let mut rsp = bidirectional_transfer
            .transfer_and_recv(&mut body_transfer)
            .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
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
                self.handle_icap_http_request_without_body(
                    state,
                    rsp,
                    header_size,
                    http_request,
                    ups_send_request,
                )
                .await
            }
            IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        http_request,
                        ups_send_request,
                    )
                    .await
                } else {
                    let mut bidirectional_transfer = BidirectionalRecvHttpRequest {
                        icap_reader: &mut self.icap_connection.reader,
                        copy_config: self.copy_config,
                        http_body_line_max_size: self.http_body_line_max_size,
                        http_trailer_max_size: self.http_trailer_max_size,
                        http_rsp_head_recv_timeout: self.http_rsp_head_recv_timeout,
                        http_req_add_no_via_header: self.http_req_add_no_via_header,
                        idle_checker: &self.idle_checker,
                        http_header_size: header_size,
                        icap_read_finished: false,
                    };
                    let r = bidirectional_transfer
                        .transfer(state, &mut body_transfer, http_request, ups_send_request)
                        .await?;

                    let icap_read_finished = bidirectional_transfer.icap_read_finished;
                    if body_transfer.finished() {
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
