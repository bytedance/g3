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

use hickory_proto::error::ProtoError;
use http::{header, Response, StatusCode};

pub struct HttpDnsResponse {
    rsp: Response<()>,
    content_length: Option<usize>,
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

        Ok(HttpDnsResponse {
            rsp,
            content_length,
        })
    }

    #[inline]
    pub fn content_length(&self) -> Option<usize> {
        self.content_length
    }

    #[inline]
    pub fn status(&self) -> StatusCode {
        self.rsp.status()
    }
}
