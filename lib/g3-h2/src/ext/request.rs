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
use http::{Method, Request, Uri};

use g3_http::server::HttpAdaptedRequest;

pub trait RequestExt {
    fn serialize_for_adapter(&self) -> Vec<u8>;
    fn adapt_to(self, other: &HttpAdaptedRequest) -> Self;
    fn clone_header(&self) -> Request<()>;
}

impl<T> RequestExt for Request<T> {
    fn serialize_for_adapter(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(1024);
        let method = self.method();
        let uri = self.uri();
        if let Some(pa) = uri.path_and_query() {
            if method.eq(&Method::OPTIONS) && pa.query().is_none() && pa.path().eq("/") {
                let _ = write!(buf, "OPTIONS * HTTP/2\r\n");
            } else {
                let _ = write!(buf, "{method} {pa} HTTP/2\r\n");
            }
        } else if method.eq(&Method::OPTIONS) {
            let _ = write!(buf, "OPTIONS * HTTP/2\r\n");
        } else {
            let _ = write!(buf, "{method} / HTTP/2\r\n");
        }
        for (name, value) in self.headers() {
            if matches!(name, &http::header::TE | &http::header::TRAILER) {
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

    fn adapt_to(self, other: &HttpAdaptedRequest) -> Self {
        let mut headers = other.headers.to_h2_map();
        // add hop-by-hop headers
        if let Some(v) = self.headers().get(http::header::TE) {
            headers.insert(http::header::TE, v.into());
        }
        let (mut parts, body) = self.into_parts();
        parts.method = other.method.clone();
        let mut uri_parts = other.uri.clone().into_parts();
        uri_parts.scheme = parts.uri.scheme().cloned();
        uri_parts.authority = parts.uri.authority().cloned();
        if let Ok(new_uri) = Uri::from_parts(uri_parts) {
            parts.uri = new_uri;
        }
        // keep old version
        parts.headers = headers;
        Request::from_parts(parts, body)
    }

    fn clone_header(&self) -> Request<()> {
        let (mut parts, _) = Request::new(()).into_parts();
        parts.method = self.method().clone();
        parts.uri = self.uri().clone();
        parts.version = self.version();
        parts.headers = self.headers().clone();
        Request::from_parts(parts, ())
    }
}
