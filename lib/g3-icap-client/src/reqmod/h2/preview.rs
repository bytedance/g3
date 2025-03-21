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

use std::io::{self, IoSlice, Write};

use bytes::{BufMut, Bytes};
use h2::RecvStream;
use h2::client::SendRequest;
use http::Request;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_h2::{H2StreamToChunkedTransfer, RequestExt};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse, H2ReqmodAdaptationError,
    H2RequestAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodResponsePayload;
use crate::reqmod::response::ReqmodResponse;

impl<I: IdleCheck> H2RequestAdapter<I> {
    fn build_preview_request(&self, http_header_len: usize, preview_size: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header, None);
        // do not send `Allow: 204, 206` as we don't want to accept 204/206 after 100-continue
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\nPreview: {preview_size}\r\n",
        );
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
                    .await;
            }
        };
        let initial_data_len = initial_body_data.len();
        let preview_size = initial_data_len.min(max_preview_size);
        let preview_eof = (preview_size == initial_data_len) && clt_body.is_end_stream();

        let http_header = http_request.serialize_for_adapter();
        let icap_header = self.build_preview_request(http_header.len(), preview_size);

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        write_preview_data(icap_w, &initial_body_data[0..preview_size], preview_eof)
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H2ReqmodAdaptationError::IcapServerWriteFailed)?;

        let mut rsp = ReqmodResponse::parse(
            &mut self.icap_connection.reader,
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
                        IcapErrorReason::ContinueAfterPreviewEof,
                        rsp.code,
                        rsp.reason.to_string(),
                    ));
                }

                let left_data = initial_body_data.split_off(preview_size);
                let mut body_transfer = if left_data.is_empty() {
                    H2StreamToChunkedTransfer::new(
                        &mut clt_body,
                        &mut self.icap_connection.writer,
                        self.copy_config.yield_size(),
                    )
                } else {
                    H2StreamToChunkedTransfer::with_chunk(
                        &mut clt_body,
                        &mut self.icap_connection.writer,
                        self.copy_config.yield_size(),
                        left_data,
                    )
                };

                let bidirectional_transfer = BidirectionalRecvIcapResponse {
                    icap_client: &self.icap_client,
                    icap_reader: &mut self.icap_connection.reader,
                    idle_checker: &self.idle_checker,
                };
                let rsp = bidirectional_transfer
                    .transfer_and_recv(&mut body_transfer)
                    .await?;

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
                            .map(|(rsp, body)| {
                                ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))
                            })
                    }
                }
            }
            204 => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                }
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
            n if (200..300).contains(&n) => {
                // FIXME we should stop send the pending HTTP body to ICAP server?
                self.icap_connection.mark_writer_finished();
                match rsp.payload {
                    IcapReqmodResponsePayload::NoPayload => {
                        self.icap_connection.mark_reader_finished();
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
                    IcapReqmodResponsePayload::HttpResponseWithoutBody(header_size) => self
                        .handle_icap_http_response_without_body(rsp, header_size)
                        .await
                        .map(|rsp| ReqmodAdaptationEndState::HttpErrResponse(rsp, None)),
                    IcapReqmodResponsePayload::HttpResponseWithBody(header_size) => self
                        .handle_icap_http_response_with_body(rsp, header_size)
                        .await
                        .map(|(rsp, body)| {
                            ReqmodAdaptationEndState::HttpErrResponse(rsp, Some(body))
                        }),
                }
            }
            _ => {
                self.icap_connection.mark_writer_finished();
                if rsp.payload == IcapReqmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(H2ReqmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponseForPreview,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }
}

async fn write_preview_data<W>(writer: &mut W, data: &[u8], preview_eof: bool) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    const END_SLICE_EOF: &[u8] = b"\r\n0; ieof\r\n\r\n";
    const END_SLICE_NO_EOF: &[u8] = b"\r\n0\r\n\r\n";

    let header = format!("{:x}\r\n", data.len());
    let end_slice = if preview_eof {
        END_SLICE_EOF
    } else {
        END_SLICE_NO_EOF
    };
    writer
        .write_all_vectored([
            IoSlice::new(header.as_bytes()),
            IoSlice::new(data),
            IoSlice::new(end_slice),
        ])
        .await?;
    Ok(())
}
