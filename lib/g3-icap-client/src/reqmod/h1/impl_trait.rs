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

use bytes::BufMut;
use http::{header, Method};

use g3_http::server::{HttpProxyClientRequest, HttpTransparentRequest};
use g3_http::HttpBodyType;

use super::{HttpAdaptedRequest, HttpRequestForAdaptation};

impl HttpRequestForAdaptation for HttpProxyClientRequest {
    fn method(&self) -> &Method {
        &self.method
    }

    fn body_type(&self) -> Option<HttpBodyType> {
        self.body_type()
    }

    fn serialize_for_adapter(&self) -> Vec<u8> {
        self.serialize_for_adapter()
    }

    fn append_upgrade_header(&self, buf: &mut Vec<u8>) {
        for v in self.hop_by_hop_headers.get_all(header::UPGRADE) {
            buf.put_slice(b"X-HTTP-Upgrade: ");
            buf.put_slice(v.as_bytes());
            buf.put_slice(b"\r\n");
        }
    }

    fn adapt_to_chunked(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_to_chunked(other)
    }
}

impl HttpRequestForAdaptation for HttpTransparentRequest {
    fn method(&self) -> &Method {
        &self.method
    }

    fn body_type(&self) -> Option<HttpBodyType> {
        self.body_type()
    }

    fn serialize_for_adapter(&self) -> Vec<u8> {
        self.serialize_for_adapter()
    }

    fn append_upgrade_header(&self, buf: &mut Vec<u8>) {
        for v in self.hop_by_hop_headers.get_all(header::UPGRADE) {
            buf.put_slice(b"X-HTTP-Upgrade: ");
            buf.put_slice(v.as_bytes());
            buf.put_slice(b"\r\n");
        }
    }

    fn adapt_to_chunked(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_to_chunked(other)
    }
}
