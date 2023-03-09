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

use bytes::Bytes;
use h2::server::{SendPushedResponse, SendResponse};
use h2::SendStream;
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
