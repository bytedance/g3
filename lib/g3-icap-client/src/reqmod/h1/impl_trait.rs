/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::BufMut;
use http::{Method, header};

use g3_http::HttpBodyType;
use g3_http::server::{HttpProxyClientRequest, HttpTransparentRequest};

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

    fn adapt_with_body(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_with_body(other)
    }

    fn adapt_without_body(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_without_body(other)
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

    fn adapt_with_body(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_with_body(other)
    }

    fn adapt_without_body(&self, other: HttpAdaptedRequest) -> Self {
        self.adapt_without_body(other)
    }
}
