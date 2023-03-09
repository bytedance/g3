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
    BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse, H1RespmodAdaptationError,
    HttpResponseAdapter, HttpResponseClientWriter, HttpResponseForAdaptation,
    RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::reqmod::h1::HttpRequestForAdaptation;
use crate::respmod::response::RespmodResponse;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> HttpResponseAdapter<I> {
    fn build_preview_request<H>(
        &self,
        http_req_hdr_len: usize,
        http_response: &H,
        http_rsp_hdr_len: usize,
        http_body_type: HttpBodyType,
        preview_state: &PreviewDataState,
    ) -> Vec<u8>
    where
        H: HttpResponseForAdaptation,
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
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, res-body={}\r\nPreview: {}\r\n",
            http_req_hdr_len + http_rsp_hdr_len,
            preview_state.preview_size
        );
        if http_body_type == HttpBodyType::ChunkedWithTrailer {
            http_response.append_trailer_header(&mut header);
        }
        header.put_slice(b"\r\n");
        header
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn xfer_with_preview<R, H, UR, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        http_request: &R,
        http_response: &H,
        ups_body_type: HttpBodyType,
        ups_body_io: &mut UR,
        clt_writer: &mut CW,
        preview_size: usize,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        R: HttpRequestForAdaptation,
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();
        let http_rsp_hdr_len = http_rsp_header.len();
        let preview_data = PreviewData {
            header: Some(http_rsp_header),
            body_type: ups_body_type,
            limit: preview_size,
            inner: ups_body_io,
        };
        let (http_rsp_payload, preview_state) = match tokio::time::timeout(
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
                        http_response,
                        ups_body_type,
                        ups_body_io,
                        clt_writer,
                    )
                    .await
            }
        };
        let icap_header = self.build_preview_request(
            http_req_header.len(),
            http_response,
            http_rsp_hdr_len,
            ups_body_type,
            &preview_state,
        );

        let icap_w = &mut self.icap_connection.0;
        icap_w
            .write_all(&icap_header)
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_req_header)
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .write_all(&http_rsp_payload)
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;

        let rsp = RespmodResponse::parse(
            &mut self.icap_connection.1,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            100 => {
                if preview_state.preview_eof {
                    return Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let mut body_transfer = ChunkedTransfer::new_after_preview(
                    ups_body_io,
                    &mut self.icap_connection.0,
                    ups_body_type,
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
                            clt_writer,
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
                                clt_writer,
                            )
                            .await
                        } else {
                            let icap_keepalive = rsp.keep_alive;
                            let bidirectional_transfer = BidirectionalRecvHttpResponse {
                                icap_rsp: rsp,
                                icap_reader: &mut self.icap_connection.1,
                                http_body_line_max_size: self.http_body_line_max_size,
                                copy_config: self.copy_config,
                                idle_checker: &self.idle_checker,
                            };
                            let r = bidirectional_transfer
                                .transfer(
                                    state,
                                    &mut body_transfer,
                                    header_size,
                                    http_response,
                                    clt_writer,
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
                    ups_body_io,
                    ups_body_type,
                    clt_writer,
                )
                .await
            }
            206 => Err(H1RespmodAdaptationError::NotImplemented("ICAP-REQMOD-206")),
            n if (200..300).contains(&n) => {
                if preview_state.preview_eof {
                    ups_body_io.consume(preview_state.consume_size);
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
                            clt_writer,
                        )
                        .await
                    }
                    IcapRespmodResponsePayload::HttpResponseWithBody(header_size) => {
                        self.handle_icap_http_response_with_body_after_transfer(
                            state,
                            rsp,
                            header_size,
                            http_response,
                            clt_writer,
                        )
                        .await
                    }
                }
            }
            _ => {
                if rsp.keep_alive && rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_client.save_connection(self.icap_connection).await;
                }
                Err(H1RespmodAdaptationError::IcapServerErrorResponse(
                    rsp.code, rsp.reason,
                ))
            }
        }
    }
}
