/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::Bytes;
use h2::SendStream;
use h2::server::{SendPushedResponse, SendResponse};
use http::Response;

use super::H2SendResponseToClient;

impl H2SendResponseToClient for SendResponse<Bytes> {
    fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<Bytes>, h2::Error> {
        self.send_response(response, end_of_stream)
    }
}

impl H2SendResponseToClient for SendPushedResponse<Bytes> {
    fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<Bytes>, h2::Error> {
        self.send_response(response, end_of_stream)
    }
}
