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

use std::str::FromStr;

use bytes::{Buf, BufMut, BytesMut};
use hickory_proto::error::ProtoError;
use hickory_proto::op::Message;
use hickory_proto::xfer::DnsResponse;
use http::{header, Response};

pub struct HttpDnsResponse {
    rsp: Response<()>,
    content_length: Option<usize>,
    body: BytesMut,
}

impl HttpDnsResponse {
    pub fn new(rsp: Response<()>) -> Result<Self, ProtoError> {
        let headers = rsp.headers();

        if let Some(ct) = headers.get(header::CONTENT_TYPE) {
            if ct.as_bytes() != super::MIME_APPLICATION_DNS.as_bytes() {
                return Err(ProtoError::from(format!(
                    "unsupported ContentType, should be {}",
                    super::MIME_APPLICATION_DNS
                )));
            }
        }

        let content_length = if let Some(cl) = headers.get(header::CONTENT_LENGTH) {
            let s = cl
                .to_str()
                .map_err(|e| ProtoError::from(format!("invalid Content-Length header: {e}")))?;
            let len = usize::from_str(s)
                .map_err(|e| ProtoError::from(format!("invalid Content-Length header: {e}")))?;
            Some(len)
        } else {
            None
        };

        // TODO: what is a good max here?
        // clamp(512, 4096) says make sure it is at least 512 bytes, and min 4096 says it is at most 4k
        // just a little protection from malicious actors.
        let response_bytes =
            BytesMut::with_capacity(content_length.unwrap_or(512).clamp(512, 4096));

        Ok(HttpDnsResponse {
            rsp,
            content_length,
            body: response_bytes,
        })
    }

    pub fn push_body<T: Buf>(&mut self, buf: T) {
        self.body.put(buf);
    }

    pub fn body_end(&self) -> bool {
        if let Some(content_length) = self.content_length {
            if self.body.len() >= content_length {
                return true;
            }
        }
        false
    }

    pub fn into_dns_response(self) -> Result<DnsResponse, ProtoError> {
        // assert the length
        if let Some(content_length) = self.content_length {
            if self.body.len() != content_length {
                // TODO: make explicit error type
                return Err(ProtoError::from(format!(
                    "expected byte length: {}, got: {}",
                    content_length,
                    self.body.len()
                )));
            }
        }

        // Was it a successful request?
        if !self.rsp.status().is_success() {
            let error_string = String::from_utf8_lossy(self.body.as_ref());

            // TODO: make explicit error type
            return Err(ProtoError::from(format!(
                "http unsuccessful code: {}, message: {}",
                self.rsp.status(),
                error_string
            )));
        }

        // and finally convert the bytes into a DNS message
        let message = Message::from_vec(&self.body)?;
        Ok(DnsResponse::new(message, self.body.to_vec()))
    }
}
