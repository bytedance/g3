/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use hickory_proto::ProtoError;
use http::uri::{Authority, Parts, PathAndQuery, Scheme};
use http::{HeaderValue, Method, Request, Uri, Version, header};

pub struct HttpDnsRequestBuilder {
    pre_built_req: Request<()>,
}

impl HttpDnsRequestBuilder {
    pub fn new(version: Version, host: &str) -> Result<Self, ProtoError> {
        let mut parts = Parts::default();
        parts.scheme = Some(Scheme::HTTPS);
        parts.authority = Some(
            Authority::from_str(host)
                .map_err(|e| ProtoError::from(format!("invalid authority: {e}")))?,
        );
        parts.path_and_query = Some(PathAndQuery::from_static(super::DNS_QUERY_PATH));

        let url = Uri::from_parts(parts)
            .map_err(|e| ProtoError::from(format!("uri parse error: {e}")))?;

        let request = Request::builder()
            .method(Method::POST)
            .uri(url)
            .version(version)
            .header(header::CONTENT_TYPE, super::MIME_APPLICATION_DNS)
            .header(header::ACCEPT, super::MIME_APPLICATION_DNS)
            .body(())
            .map_err(|e| ProtoError::from(format!("http stream errored: {e}")))?;

        Ok(HttpDnsRequestBuilder {
            pre_built_req: request,
        })
    }

    pub fn post(&self, content_length: usize) -> Request<()> {
        let mut req = self.pre_built_req.clone();
        req.headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from(content_length));
        req
    }
}
