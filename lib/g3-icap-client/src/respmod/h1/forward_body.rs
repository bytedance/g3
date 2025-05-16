/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use tokio::io::AsyncBufRead;

use g3_http::{H1BodyToChunkedTransfer, HttpBodyType};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    BidirectionalRecvHttpResponse, BidirectionalRecvIcapResponse, H1RespmodAdaptationError,
    HttpResponseAdapter, HttpResponseClientWriter, HttpResponseForAdaptation,
    RespmodAdaptationEndState, RespmodAdaptationRunState,
};
use crate::reqmod::h1::HttpRequestForAdaptation;
use crate::respmod::IcapRespmodResponsePayload;

impl<I: IdleCheck> HttpResponseAdapter<I> {
    fn build_forward_all_request(
        &self,
        http_req_hdr_len: usize,
        http_rsp_hdr_len: usize,
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(self.icap_client.partial_request_header.len() + 128);
        header.extend_from_slice(&self.icap_client.partial_request_header);
        self.push_extended_headers(&mut header);
        let _ = write!(
            header,
            "Encapsulated: req-hdr=0, res-hdr={http_req_hdr_len}, res-body={}\r\n",
            http_req_hdr_len + http_rsp_hdr_len
        );
        header.put_slice(b"\r\n");
        header
    }

    pub(super) async fn xfer_without_preview<R, H, UR, CW>(
        mut self,
        state: &mut RespmodAdaptationRunState,
        http_request: &R,
        http_response: &H,
        ups_body_type: HttpBodyType,
        ups_body_io: &mut UR,
        clt_writer: &mut CW,
    ) -> Result<RespmodAdaptationEndState<H>, H1RespmodAdaptationError>
    where
        R: HttpRequestForAdaptation,
        H: HttpResponseForAdaptation,
        UR: AsyncBufRead + Unpin,
        CW: HttpResponseClientWriter<H> + Unpin,
    {
        let http_req_header = http_request.serialize_for_adapter();
        let http_rsp_header = http_response.serialize_for_adapter();
        let icap_header =
            self.build_forward_all_request(http_req_header.len(), http_rsp_header.len());

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_req_header),
                IoSlice::new(&http_rsp_header),
            ])
            .await
            .map_err(H1RespmodAdaptationError::IcapServerWriteFailed)?;

        let mut body_transfer = H1BodyToChunkedTransfer::new(
            ups_body_io,
            &mut self.icap_connection.writer,
            ups_body_type,
            self.http_body_line_max_size,
            self.copy_config,
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
            state.mark_ups_recv_all();
        }

        match rsp.payload {
            IcapRespmodResponsePayload::NoPayload => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
                self.icap_connection.mark_reader_finished();
                self.handle_icap_ok_without_payload(rsp).await
            }
            IcapRespmodResponsePayload::HttpResponseWithoutBody(header_size) => {
                if body_transfer.finished() {
                    self.icap_connection.mark_writer_finished();
                }
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
                    self.icap_connection.mark_writer_finished();
                    self.handle_icap_http_response_with_body_after_transfer(
                        state,
                        rsp,
                        header_size,
                        http_response,
                        clt_writer,
                    )
                    .await
                } else {
                    let mut bidirectional_transfer = BidirectionalRecvHttpResponse {
                        http_body_line_max_size: self.http_body_line_max_size,
                        copy_config: self.copy_config,
                        idle_checker: &self.idle_checker,
                        http_header_size: header_size,
                        icap_read_finished: false,
                    };
                    let r = bidirectional_transfer
                        .transfer(
                            state,
                            &mut body_transfer,
                            http_response,
                            &mut self.icap_connection.reader,
                            clt_writer,
                        )
                        .await?;
                    if body_transfer.finished() {
                        state.mark_ups_recv_all();
                        self.icap_connection.mark_writer_finished();
                        if bidirectional_transfer.icap_read_finished {
                            self.icap_connection.mark_reader_finished();
                            if rsp.keep_alive {
                                self.icap_client.save_connection(self.icap_connection);
                            }
                        }
                    }
                    Ok(r)
                }
            }
        }
    }
}
