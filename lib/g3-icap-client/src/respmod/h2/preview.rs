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

use bytes::BufMut;
use h2::RecvStream;
use http::{Request, Response};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_h2::{H2StreamToChunkedTransfer, RequestExt, ResponseExt};
use g3_io_ext::IdleCheck;

use super::{
    BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse, H2RespmodAdaptationError,
    H2ResponseAdapter, H2SendResponseToClient, RespmodAdaptationEndState,
    RespmodAdaptationRunState,
};
use crate::respmod::response::RespmodResponse;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> H2ResponseAdapter<I> {
    fn build_preview_request(
        &self,
        http_req_hdr_len: usize,
        http_response: &Response<()>,
        http_rsp_hdr_len: usize,
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
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, res-body={}\r\nPreview: {preview_size}\r\n",
            http_req_hdr_len + http_rsp_hdr_len,
        );
        for trailer in http_response.headers().get_all(http::header::TRAILER) {
            header.put_slice(b"Trailer: ");
            header.put_slice(trailer.as_bytes());
            header.put_slice(b"\r\n");
        }
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_with_preview<CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        http_request: &Request<()>,
        http_response: Response<()>,
        mut ups_body: RecvStream,
        clt_send_response: &mut CW,
        max_preview_size: usize,
    ) -> Result<RespmodAdaptationEndState, H2RespmodAdaptationError>
    where
        CW: H2SendResponseToClient,
    {
        let mut initial_body_data = match tokio::time::timeout(
            self.icap_client.config.preview_data_read_timeout,
            ups_body.data(),
        )
        .await
        {
            Ok(Some(Ok(data))) => data,
            Ok(Some(Err(e))) => {
                return Err(H2RespmodAdaptationError::HttpUpstreamRecvDataFailed(e))
            }
            Ok(None) | Err(_) => {
                return self
                    .xfer_without_preview(
                        state,
                        http_request,
                        http_response,
                        ups_body,
                        clt_send_response,
                    )
                    .await
            }
        };
        let initial_data_len = initial_body_data.len();
        let preview_size = initial_data_len.min(max_preview_size);
        let preview_eof = (preview_size == initial_data_len) && ups_body.is_end_stream();

        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();
        let icap_header = self.build_preview_request(
            http_req_header.len(),
            &http_response,
            http_rsp_header.len(),
            preview_size,
        );
        let has_trailer = http_request.headers().contains_key(http::header::TRAILER);

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
        write_preview_data(icap_w, &initial_body_data[0..preview_size])
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
            100 => {
                if preview_eof {
                    return Err(H2RespmodAdaptationError::IcapServerErrorResponse(
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let left_data = initial_body_data.split_off(preview_size);
                let mut body_transfer = if left_data.is_empty() {
                    H2StreamToChunkedTransfer::new(
                        &mut ups_body,
                        &mut self.icap_connection.0,
                        has_trailer,
                        self.copy_config.yield_size(),
                    )
                } else {
                    H2StreamToChunkedTransfer::with_chunk(
                        &mut ups_body,
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
                if body_transfer.finished() {
                    state.mark_ups_recv_all();
                }

                match rsp.payload {
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
                        if body_transfer.finished() {
                            self.handle_icap_http_response_with_body_after_transfer(
                                state,
                                rsp,
                                header_size,
                                http_response,
                                clt_send_response,
                            )
                            .await
                        } else {
                            let icap_keepalive = rsp.keep_alive;
                            let bidirectional_transfer = BidirectionalRecvHttpResponse {
                                icap_rsp: rsp,
                                icap_reader: &mut self.icap_connection.1,
                                copy_config: self.copy_config,
                                http_body_line_max_size: self.http_body_line_max_size,
                                http_trailer_max_size: self.http_trailer_max_size,
                                idle_checker: &self.idle_checker,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    header_size,
                                    http_response,
                                    clt_send_response,
                                )
                                .await?;
                            if body_transfer.finished() {
                                state.mark_ups_recv_all();
                            }
                            if icap_keepalive && state.icap_io_finished {
                                self.icap_client.save_connection(self.icap_connection).await;
                            }
                            Ok(r)
                        }
                    }
                }
            }
            204 => {
                self.handle_original_http_response_with_body(
                    state,
                    rsp,
                    http_response,
                    initial_body_data,
                    ups_body,
                    clt_send_response,
                )
                .await
            }
            206 => Err(H2RespmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
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
