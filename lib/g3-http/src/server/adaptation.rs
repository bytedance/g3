/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use http::{HeaderName, Method, Uri, Version};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use super::HttpRequestParseError;
use crate::{HttpHeaderLine, HttpLineParseError, HttpMethodLine};

pub struct HttpAdaptedRequest {
    pub method: Method,
    pub uri: Uri,
    pub version: Version,
    pub headers: HttpHeaderMap,
    pub content_length: Option<u64>,
}

impl HttpAdaptedRequest {
    fn new(method: Method, uri: Uri, version: Version) -> Self {
        HttpAdaptedRequest {
            method,
            uri,
            version,
            headers: HttpHeaderMap::default(),
            content_length: None,
        }
    }

    pub async fn parse<R>(
        reader: &mut R,
        header_size: usize,
        ignore_via: bool,
    ) -> Result<Self, HttpRequestParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut read_size: usize = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', header_size, &mut line_buf)
            .await?;
        if nr == 0 {
            return Err(HttpRequestParseError::ClientClosed);
        }
        if !found {
            return if nr < header_size {
                Err(HttpRequestParseError::ClientClosed)
            } else {
                Err(HttpRequestParseError::TooLargeHeader(header_size))
            };
        }
        read_size += nr;

        let mut req = HttpAdaptedRequest::build_from_method_line(&line_buf)?;

        loop {
            if read_size >= header_size {
                return Err(HttpRequestParseError::TooLargeHeader(header_size));
            }
            line_buf.clear();
            let max_len = header_size - read_size;
            let (found, nr) = reader
                .limited_read_until(b'\n', max_len, &mut line_buf)
                .await?;
            if nr == 0 {
                return Err(HttpRequestParseError::ClientClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(HttpRequestParseError::ClientClosed)
                } else {
                    Err(HttpRequestParseError::TooLargeHeader(header_size))
                };
            }
            read_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            req.parse_header_line(&line_buf, ignore_via)?;
        }

        Ok(req)
    }

    fn build_from_method_line(line_buf: &[u8]) -> Result<Self, HttpRequestParseError> {
        let req =
            HttpMethodLine::parse(line_buf).map_err(HttpRequestParseError::InvalidMethodLine)?;

        let version = match req.version {
            0 => Version::HTTP_10,
            1 => Version::HTTP_11,
            2 => Version::HTTP_2,
            _ => unreachable!(),
        };

        let method = Method::from_str(req.method)
            .map_err(|_| HttpRequestParseError::UnsupportedMethod(req.method.to_string()))?;
        let uri =
            Uri::from_str(req.uri).map_err(|_| HttpRequestParseError::InvalidRequestTarget)?;
        Ok(HttpAdaptedRequest::new(method, uri, version))
    }

    fn parse_header_line(
        &mut self,
        line_buf: &[u8],
        ignore_via: bool,
    ) -> Result<(), HttpRequestParseError> {
        let header =
            HttpHeaderLine::parse(line_buf).map_err(HttpRequestParseError::InvalidHeaderLine)?;
        self.handle_header(header, ignore_via)
    }

    fn handle_header(
        &mut self,
        header: HttpHeaderLine,
        ignore_via: bool,
    ) -> Result<(), HttpRequestParseError> {
        let name = HeaderName::from_str(header.name).map_err(|_| {
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        })?;

        match name.as_str() {
            "connection" | "keep-alive" | "te" => {
                // ignored hop-by-hop options
                return Ok(());
            }
            "content-length" => {
                let content_length = u64::from_str(header.value)
                    .map_err(|_| HttpRequestParseError::InvalidContentLength)?;
                self.content_length = Some(content_length);
            }
            "transfer-encoding" => {
                // this will always be chunked encoding
                return Ok(());
            }
            "via" => {
                if ignore_via {
                    return Ok(());
                }
            }
            _ => {}
        }

        let mut value = HttpHeaderValue::from_str(header.value).map_err(|_| {
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        value.set_original_name(header.name);
        self.headers.append(name, value);
        Ok(())
    }
}
