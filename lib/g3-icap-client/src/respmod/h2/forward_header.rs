/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{IoSlice, Write};

use bytes::BufMut;
use http::{Request, Response};
use tokio::io::AsyncWriteExt;

use g3_h2::{RequestExt, ResponseExt};
use g3_io_ext::{IdleCheck, LimitedWriteExt};

use super::{
    H2RespmodAdaptationError, H2ResponseAdapter, H2SendResponseToClient, RespmodAdaptationEndState,
    RespmodAdaptationRunState,
};
use crate::reason::IcapErrorReason;
use crate::respmod::IcapRespmodResponsePayload;
use crate::respmod::response::RespmodResponse;

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

        let icap_w = &mut self.icap_connection.writer;
        icap_w
            .write_all_vectored([
                IoSlice::new(&icap_header),
                IoSlice::new(&http_req_header),
                IoSlice::new(&http_rsp_header),
            ])
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;
        icap_w
            .flush()
            .await
            .map_err(H2RespmodAdaptationError::IcapServerWriteFailed)?;
        self.icap_connection.mark_writer_finished();

        let rsp = RespmodResponse::parse(
            &mut self.icap_connection.reader,
            self.icap_client.config.icap_max_header_size,
        )
        .await?;

        match rsp.code {
            204 => {
                if rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                }
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
                    self.icap_connection.mark_reader_finished();
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
                if rsp.payload == IcapRespmodResponsePayload::NoPayload {
                    self.icap_connection.mark_reader_finished();
                    if rsp.keep_alive && rsp.payload == IcapRespmodResponsePayload::NoPayload {
                        self.icap_client.save_connection(self.icap_connection);
                    }
                }
                Err(H2RespmodAdaptationError::IcapServerErrorResponse(
                    IcapErrorReason::UnknownResponse,
                    rsp.code,
                    rsp.reason,
                ))
            }
        }
    }
}
