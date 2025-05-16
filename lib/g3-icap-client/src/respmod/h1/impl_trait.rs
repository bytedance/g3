/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use http::Method;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_http::HttpBodyType;
use g3_http::client::{HttpForwardRemoteResponse, HttpTransparentResponse};

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

    fn adapt_with_body(&self, other: HttpAdaptedResponse) -> Self {
        self.adapt_with_body(other)
    }

    fn adapt_without_body(&self, other: HttpAdaptedResponse) -> Self {
        self.adapt_without_body(other)
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

    fn adapt_with_body(&self, other: HttpAdaptedResponse) -> Self {
        self.adapt_with_body(other)
    }

    fn adapt_without_body(&self, other: HttpAdaptedResponse) -> Self {
        self.adapt_without_body(other)
    }
}

impl<W, H> HttpResponseClientWriter<H> for W
where
    W: AsyncWrite + Send + Unpin,
    H: HttpResponseForAdaptation + Sync,
{
    async fn send_response_header(&mut self, req: &H) -> io::Result<()> {
        let head = req.serialize_for_client();
        self.write_all(&head).await
    }
}
