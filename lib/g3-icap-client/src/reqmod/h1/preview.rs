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
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};

use g3_http::{ChunkedTransfer, HttpBodyType, PreviewData, PreviewDataState};
use g3_io_ext::IdleCheck;

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H1ReqmodAdaptationError,
    HttpRequestAdapter, HttpRequestForAdaptation, HttpRequestUpstreamWriter,
    ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reqmod::response::ReqmodResponse;
use crate::reqmod::IcapReqmodResponsePayload;

impl<I: IdleCheck> HttpRequestAdapter<I> {
    fn build_preview_request<H>(
        &self,
        http_request: &H,
        http_header_len: usize,
        http_body_type: HttpBodyType,
        preview_state: &PreviewDataState,
    ) -> Vec<u8>
    where
        H: HttpRequestForAdaptation,
    {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        match (self.icap_options.support_204, self.icap_options.support_206) {
            (true, true) => header.put_slice(b"Allow: 204, 206\r\n"),
            (true, false) => header.put_slice(b"Allow: 204\r\n"),
            (false, true) => header.put_slice(b"Allow: 206\r\n"),
            (false, false) => {}
        }
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={}\r\nPreview: {}\r\n",
            http_header_len, preview_state.preview_size
        );
        if http_body_type == HttpBodyType::ChunkedWithTrailer {
            http_request.append_trailer_header(&mut header);
        }
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_with_preview<H, CR, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: &H,
        clt_body_type: HttpBodyType,
        clt_body_io: &mut CR,
        ups_writer: &mut UW,
        preview_size: usize,
    ) -> Result<ReqmodAdaptationEndState<H>, H1ReqmodAdaptationError>
    where
        H: HttpRequestForAdaptation,
        CR: AsyncBufRead + Unpin,
        UW: HttpRequestUpstreamWriter<H> + Unpin,
    {
        let http_header = http_request.serialize_for_adapter();
        let header_len = http_header.len();
        let preview_data = PreviewData {
            header: Some(http_header),
            body_type: clt_body_type,
            limit: preview_size,
            inner: clt_body_io,
        };
        let (http_payload, preview_state) = match tokio::time::timeout(
            self.icap_client.config.preview_data_read_timeout,
            preview_data,
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                return self
                    .xfer_without_preview(
                        state,
                        http_request,
                        clt_body_type,
                        clt_body_io,
                        ups_writer,
                    )
                    .await
            }
        };
        let icap_header =
            self.build_preview_request(http_request, header_len, clt_body_type, &preview_state);

        let icap_w = &mut self.icap_connection.0;
        icap_w
            .write_all(&icap_header)
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_payload)
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1ReqmodAdaptationError::IcapServerWriteFailed)?;

        let mut rsp = ReqmodResponse::parse(
            &mut self.icap_connection.1,
            self.icap_client.config.icap_max_header_size,
            &self.icap_client.config.respond_shared_names,
        )
        .await?;
        let shared_headers = rsp.take_shared_headers();
        if !shared_headers.is_empty() {
            state.respond_shared_headers = Some(shared_headers);
        }

        match rsp.code {
            100 => {
                if preview_state.preview_eof {
                    return Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let mut body_transfer = ChunkedTransfer::new_after_preview(
                    clt_body_io,
                    &mut self.icap_connection.0,
                    clt_body_type,
                    self.http_body_line_max_size,
                    self.copy_config,
                    preview_state,
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
                    IcapReqmodResponsePayload::NoPayload => {
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
                        if body_transfer.finished() {
                            self.handle_icap_http_request_with_body_after_transfer(
                                state,
                                rsp,
                                header_size,
                                http_request,
                                ups_writer,
                            )
                            .await
                        } else {
                            let icap_keepalive = rsp.keep_alive;
                            let bidirectional_transfer = BidirectionalRecvHttpRequest {
                                icap_rsp: rsp,
                                icap_reader: &mut self.icap_connection.1,
                                http_body_line_max_size: self.http_body_line_max_size,
                                http_req_add_no_via_header: self.http_req_add_no_via_header,
                                copy_config: self.copy_config,
                                idle_checker: &self.idle_checker,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    header_size,
                                    http_request,
                                    ups_writer,
                                )
                                .await?;
                            if body_transfer.finished() {
                                state.clt_read_finished = true;
                            }
                            if icap_keepalive && state.icap_io_finished {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(r)
                        }
                    }
                    IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                        self.handle_icap_http_response_without_body(rsp, header_size)
                            .await
                    }
                    IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => {
                        self.handle_icap_http_response_with_body(rsp, header_size)
                            .await
                    }
                }
            }
            204 => {
                self.handle_original_http_request_with_body(
                    state,
                    rsp,
                    http_request,
                    clt_body_io,
                    clt_body_type,
                    ups_writer,
                )
                .await
            }
            206 => Err(H1ReqmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                if preview_state.preview_eof {
                    clt_body_io.consume(preview_state.consume_size);
                    state.clt_read_finished = true;
                }
                match rsp.payload {
                    IcapReqmodResponsePayload::NoPayload => {
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
                    IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                        self.handle_icap_http_response_without_body(rsp, header_size)
                            .await
                    }
                    IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => {
                        self.handle_icap_http_response_with_body(rsp, header_size)
                            .await
                    }
                }
            }
            _ => {
                if rsp.keep_alive && rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_client.save_connection(self.icap_connection).await;
                }
                Err(H1ReqmodAdaptationError::IcapServerErrorResponse(
                    rsp.code, rsp.reason,
                ))
            }
        }
    }
}
