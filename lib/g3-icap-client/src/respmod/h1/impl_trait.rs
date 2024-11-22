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

use http::Method;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_http::client::{HttpForwardRemoteResponse, HttpTransparentResponse};
use g3_http::HttpBodyType;

use super::{HttpAdaptedResponse, HttpResponseClientWriter, HttpResponseForAdaptation};

impl HttpResponseForAdaptation for HttpForwardRemoteResponse {
    fn body_type(&self, method: &Method) -> Option<HttpBodyType> {
        self.body_type(method)
    }

    fn serialize_for_client(&self) -> Vec<u8> {
        self.serialize()
    }

    fn serialize_for_adapter(&self) -> Vec<u8> {
        self.serialize_for_adapter()
    }

    fn adapt_to_chunked(&self, other: HttpAdaptedResponse) -> Self {
        self.adapt_to_chunked(other)
    }
}

impl HttpResponseForAdaptation for HttpTransparentResponse {
    fn body_type(&self, method: &Method) -> Option<HttpBodyType> {
        self.body_type(method)
    }

    fn serialize_for_client(&self) -> Vec<u8> {
        self.serialize()
    }

    fn serialize_for_adapter(&self) -> Vec<u8> {
        self.serialize_for_adapter()
    }

    fn adapt_to_chunked(&self, other: HttpAdaptedResponse) -> Self {
        self.adaptat_to_chunked(other)
    }
}

impl<W, H> HttpResponseClientWriter<H> for W
where
    W: AsyncWrite + Send + Unpin,
    H: HttpResponseForAdaptation + Sync,
{
    async fn send_response_header(&mut self, req: &H) -> std::io::Result<()> {
        let head = req.serialize_for_client();
        self.write_all(&head).await
    }
}
