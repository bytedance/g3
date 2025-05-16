/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::Write;

use bytes::BufMut;
use http::uri::Authority;
use http::{HeaderMap, Method, Request, Uri};

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
                let _ = write!(buf, "OPTIONS * HTTP/1.1\r\n");
            } else {
                let _ = write!(buf, "{method} {pa} HTTP/1.1\r\n");
            }
        } else if method.eq(&Method::OPTIONS) {
            buf.extend_from_slice(b"OPTIONS * HTTP/1.1\r\n");
        } else {
            let _ = write!(buf, "{method} / HTTP/1.1\r\n");
        }
        for (name, value) in self.headers() {
            if matches!(name, &http::header::TE) {
                // skip hop-by-hop headers
                continue;
            }
            buf.put_slice(name.as_ref());
            buf.put_slice(b": ");
            buf.put_slice(value.as_bytes());
            buf.put_slice(b"\r\n");
        }
        if !self.headers().contains_key(http::header::HOST) {
            if let Some(host) = uri.host() {
                buf.put_slice(b"Host: ");
                buf.put_slice(host.as_bytes());
                buf.put_slice(b"\r\n");
            }
        }
        buf.put_slice(b"\r\n");
        buf
    }

    fn adapt_to(self, other: &HttpAdaptedRequest) -> Self {
        let mut headers = HeaderMap::from(&other.headers);
        // add hop-by-hop headers
        if let Some(v) = self.headers().get(http::header::TE) {
            headers.insert(http::header::TE, v.into());
        }
        let (mut parts, body) = self.into_parts();
        parts.method = other.method.clone();
        let mut uri_parts = other.uri.clone().into_parts();
        uri_parts.scheme = parts.uri.scheme().cloned();
        uri_parts.authority = parts.uri.authority().cloned();
        if let Some(host) = headers.remove(http::header::HOST) {
            // we should always remove the Host header to be compatible with Google,
            // but let's keep the same as client behaviour here
            if parts.headers.contains_key(http::header::HOST) {
                headers.insert(http::header::HOST, host.clone());
            }
            if uri_parts.authority.is_none() {
                if let Ok(authority) = Authority::from_maybe_shared(host.clone()) {
                    //update the authority field
                    uri_parts.authority = Some(authority);
                }
            }
        }
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
