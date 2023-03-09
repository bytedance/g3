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
use http::Response;

use g3_http::client::HttpAdaptedResponse;

pub trait ResponseExt {
    fn serialize_for_adapter(&self) -> Vec<u8>;
    fn adapt_to(self, other: &HttpAdaptedResponse) -> Self;
}

impl<T> ResponseExt for Response<T> {
    fn serialize_for_adapter(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(1024);

        let status = self.status();
        let reason = self
            .status()
            .canonical_reason()
            .unwrap_or("NOT STANDARD STATUS CODE");
        let _ = write!(buf, "HTTP/2 {} {}\r\n", status.as_u16(), reason,);

        for (name, value) in self.headers() {
            if matches!(name, &http::header::TRAILER) {
                // skip hop-by-hop headers
                continue;
            }
            buf.put_slice(name.as_ref());
            buf.put_slice(b": ");
            buf.put_slice(value.as_bytes());
            buf.put_slice(b"\r\n");
        }
        buf.put_slice(b"\r\n");
        buf
    }

    fn adapt_to(self, other: &HttpAdaptedResponse) -> Self {
        let (mut parts, body) = self.into_parts();
        // keep old version
        parts.status = other.status;
        parts.headers = other.headers.clone();
        Response::from_parts(parts, body)
    }
}
