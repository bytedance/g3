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
use tokio::io::{AsyncRead, AsyncWrite, BufWriter};

use g3_http::StreamToChunkedTransfer;
use g3_io_ext::{IdleCheck, LimitedWriteExt};
use g3_smtp_proto::command::{MailParam, RecipientParam};
use g3_smtp_proto::io::TextDataDecodeReader;

use super::{HttpAdapterErrorResponse, SmtpAdaptationError, SmtpMessageAdapter};
use crate::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use crate::reqmod::IcapReqmodResponsePayload;

mod bidirectional;
use bidirectional::{BidirectionalRecvHttpRequest, BidirectionalRecvIcapResponse};

mod recv_request;
mod recv_response;

impl<I: IdleCheck> SmtpMessageAdapter<I> {
    fn build_forward_all_request(&self, http_header_len: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 64);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, req-body={http_header_len}\r\n",
        );
        header.put_slice(b"\r\n");
        header
    }

    pub async fn xfer_data_without_preview<CR, UW>(
        mut self,
        state: &mut ReqmodAdaptationRunState,
        clt_r: &mut CR,
        ups_w: &mut UW,
        mail_from: &MailParam,
        mail_to: &[RecipientParam],
    ) -> Result<ReqmodAdaptationEndState, SmtpAdaptationError>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let http_header = self.build_http_header(mail_from, mail_to);
        let icap_header = self.build_forward_all_request(http_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([IoSlice::new(&icap_header), IoSlice::new(&http_header)])
            .await
            .map_err(SmtpAdaptationError::IcapServerWriteFailed)?;

        let mut message_reader = TextDataDecodeReader::new(clt_r, self.copy_config.buffer_size());
        let mut icap_buf_writer = BufWriter::new(&mut self.icap_connection.writer);
        let mut body_transfer = StreamToChunkedTransfer::new_with_no_trailer(
            &mut message_reader,
            &mut icap_buf_writer,
            self.copy_config.yield_size(),
        );

        let bidirectional_transfer = BidirectionalRecvIcapResponse {
            icap_client: &self.icap_client,
            icap_reader: &mut self.icap_connection.reader,
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
                self.handle_icap_http_request_without_body(state, rsp, header_size)
                    .await
            }
            IcapReqmodResponsePayload::HttpRequestWithBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                    self.handle_icap_http_request_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        ups_w,
                    )
                    .await
                } else {
                    let mut bidirectional_transfer = BidirectionalRecvHttpRequest {
                        icap_reader: &mut self.icap_connection.reader,
                        copy_config: self.copy_config,
                        idle_checker: &self.idle_checker,
                        http_header_size: header_size,
                        icap_read_finished: false,
                    };
                    let r = bidirectional_transfer
                        .transfer(state, &mut body_transfer, ups_w)
                        .await?;
                    let icap_read_finished = bidirectional_transfer.icap_read_finished;
                    if body_transfer.finished() {
                        if message_reader.finished() {
                            state.clt_read_finished = true;
                        }
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
