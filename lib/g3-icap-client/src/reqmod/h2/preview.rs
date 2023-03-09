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

use std::io::{self, Write};

use bytes::{BufMut, Bytes};
use h2::client::SendRequest;
use h2::RecvStream;
use http::Request;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_h2::{H2StreamToChunkedTransfer, RequestExt};
use g3_io_ext::IdleCheck;

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H2ReqmodAdaptationError,
    H2RequestAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reqmod::response::ReqmodResponse;
use crate::reqmod::IcapReqmodResponsePayload;

impl<I: IdleCheck> H2RequestAdapter<I> {
    fn build_preview_request(
        &self,
        http_request: &Request<()>,
        http_header_len: usize,
        preview_size: usize,
    ) -> Vec<u8> {
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
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\nPreview: {preview_size}\r\n",
        );
        for trailer in http_request.headers().get_all(http::header::TRAILER) {
            header.put_slice(b"Trailer: ");
            header.put_slice(trailer.as_bytes());
            header.put_slice(b"\r\n");
        }
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_with_preview(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        http_request: Request<()>,
        mut clt_body: RecvStream,
        ups_send_request: SendRequest<Bytes>,
        max_preview_size: usize,
    ) -> Result<ReqmodAdaptationEndState, H2ReqmodAdaptationError> {
        let mut initial_body_data = match tokio::time::timeout(
            self.icap_client.config.preview_data_read_timeout,
            clt_body.data(),
        )
        .await
        {
            Ok(Some(Ok(data))) => data,
            Ok(Some(Err(e))) => return Err(H2ReqmodAdaptationError::HttpClientRecvDataFailed(e)),
            Ok(None) | Err(_) => {
                return self
                    .xfer_without_preview(state, http_request, clt_body, ups_send_request)
                    .await
            }
        };
        let initial_data_len = initial_body_data.len();
        let preview_size = initial_data_len.min(max_preview_size);
        let preview_eof = (preview_size == initial_data_len) && clt_body.is_end_stream();

        let http_header = http_request.serialize_for_adapter();
        let icap_header =
            self.build_preview_request(&http_request, http_header.len(), preview_size);
        let has_trailer = http_request.headers().contains_key(http::header::TRAILER);

        let icap_w = &mut self.icap_connection.0;
        icap_w
            .write_all(&icap_header)
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_header)
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        write_preview_data(icap_w, &initial_body_data[0..preview_size])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

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
                if preview_eof {
                    return Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let left_data = initial_body_data.split_off(preview_size);
                let mut body_transfer = if left_data.is_empty() {
                    H2StreamToChunkedTransfer::new(
                        &mut clt_body,
                        &mut self.icap_connection.0,
                        has_trailer,
                        self.copy_config.yield_size(),
                    )
                } else {
                    H2StreamToChunkedTransfer::with_chunk(
                        &mut clt_body,
                        &mut self.icap_connection.0,
                        has_trailer,
                        self.copy_config.yield_size(),
                        initial_body_data,
                    )
                };

                let bidirectional_transfer = BidirectionalRecvIcapResponse {
                    icap_client: &self.icap_client,
                    icap_reader: &mut self.icap_connection.1,
                    idle_checker: &self.idle_checker,
                };
                let rsp = bidirectional_transfer
                    .transfer_and_recv(&mut body_transfer)
                    .await?;

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
                            ups_send_request,
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
                                ups_send_request,
                            )
                            .await
                        } else {
                            let icap_keepalive = rsp.keep_alive;
                            let bidirectional_transfer = BidirectionalRecvHttpRequest {
                                icap_rsp: rsp,
                                icap_reader: &mut self.icap_connection.1,
                                copy_config: self.copy_config,
                                http_body_line_max_size: self.http_body_line_max_size,
                                http_trailer_max_size: self.http_trailer_max_size,
                                http_rsp_head_recv_timeout: self.http_rsp_head_recv_timeout,
                                http_req_add_no_via_header: self.http_req_add_no_via_header,
                                idle_checker: &self.idle_checker,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    header_size,
                                    http_request,
                                    ups_send_request,
                                )
                                .await?;

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
                    initial_body_data,
                    clt_body,
                    ups_send_request,
                )
                .await
            }
            206 => Err(H2ReqmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => match rsp.payload {
                IcapReqmodResponsePayload::NoPayload => {
                    self.handle_icap_ok_without_payload(rsp).await
                }
                IcapReqmodResponsePayload::HttpRequestWithoutBody(header_size) => {
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
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        http_request,
                        ups_send_request,
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
            },
            _ => {
                if rsp.keep_alive && rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_client.save_connection(self.icap_connection).await;
                }
                Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    rsp.code, rsp.reason,
                ))
            }
        }
    }
}

async fn write_preview_data<W>(writer: &mut W, data: &[u8]) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let header = format!("{:x}\r\n", data.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(data).await?;
    writer.write_all(b"\r\n0\r\n\r\n").await?;
    Ok(())
}
