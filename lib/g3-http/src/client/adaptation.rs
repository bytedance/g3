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

use std::str::FromStr;

use http::{HeaderName, StatusCode, Version};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use super::HttpResponseParseError;
use crate::{HttpHeaderLine, HttpLineParseError, HttpStatusLine};

pub struct HttpAdaptedResponse {
    pub version: Version,
    pub status: StatusCode,
    pub reason: String,
    pub headers: HttpHeaderMap,
    trailer: Vec<HttpHeaderValue>,
}

impl HttpAdaptedResponse {
    fn new(version: Version, status: StatusCode, reason: String) -> Self {
        HttpAdaptedResponse {
            version,
            status,
            reason,
            headers: HttpHeaderMap::default(),
            trailer: Vec::new(),
        }
    }

    pub fn set_chunked_encoding(&mut self) {
        self.headers.insert(
            http::header::TRANSFER_ENCODING,
            HttpHeaderValue::from_static("chunked"),
        );
    }

    pub fn set_trailer(&mut self, trailers: Vec<HttpHeaderValue>) {
        self.trailer = trailers;
    }

    #[inline]
    pub(crate) fn trailer(&self) -> &[HttpHeaderValue] {
        &self.trailer
    }

    pub async fn parse<R>(
        reader: &mut R,
        header_size: usize,
    ) -> Result<Self, HttpResponseParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut read_size: usize = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', header_size, &mut line_buf)
            .await?;
        if nr == 0 {
            return Err(HttpResponseParseError::RemoteClosed);
        }
        if !found {
            return if nr < header_size {
                Err(HttpResponseParseError::RemoteClosed)
            } else {
                Err(HttpResponseParseError::TooLargeHeader(header_size))
            };
        }
        read_size += nr;

        let mut rsp = HttpAdaptedResponse::build_from_status_line(&line_buf)?;

        loop {
            if read_size >= header_size {
                return Err(HttpResponseParseError::TooLargeHeader(header_size));
            }
            line_buf.clear();
            let max_len = header_size - read_size;
            let (found, nr) = reader
                .limited_read_until(b'\n', max_len, &mut line_buf)
                .await?;
            if nr == 0 {
                return Err(HttpResponseParseError::RemoteClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(HttpResponseParseError::RemoteClosed)
                } else {
                    Err(HttpResponseParseError::TooLargeHeader(header_size))
                };
            }
            read_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            rsp.parse_header_line(&line_buf)?;
        }

        Ok(rsp)
    }

    fn build_from_status_line(line_buf: &[u8]) -> Result<Self, HttpResponseParseError> {
        let rsp =
            HttpStatusLine::parse(line_buf).map_err(HttpResponseParseError::InvalidStatusLine)?;
        let version = match rsp.version {
            0 => Version::HTTP_10,
            1 => Version::HTTP_11,
            2 => Version::HTTP_2,
            _ => unreachable!(),
        };
        let status = StatusCode::from_u16(rsp.code).map_err(|_| {
            HttpResponseParseError::InvalidStatusLine(HttpLineParseError::InvalidStatusCode)
        })?;

        Ok(HttpAdaptedResponse::new(
            version,
            status,
            rsp.reason.to_string(),
        ))
    }

    fn parse_header_line(&mut self, line_buf: &[u8]) -> Result<(), HttpResponseParseError> {
        let header =
            HttpHeaderLine::parse(line_buf).map_err(HttpResponseParseError::InvalidHeaderLine)?;
        self.handle_header(header)
    }

    fn handle_header(&mut self, header: HttpHeaderLine) -> Result<(), HttpResponseParseError> {
        let name = HeaderName::from_str(header.name).map_err(|_| {
            HttpResponseParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        })?;

        match name.as_str() {
            "connection" | "keep-alive" | "trailer" => {
                // ignored hop-by-hop options
                return Ok(());
            }
            "transfer-encoding" | "content-length" => {
                // this will always be chunked encoding
                return Ok(());
            }
            _ => {}
        }

        let mut value = HttpHeaderValue::from_str(header.value).map_err(|_| {
            HttpResponseParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        value.set_original_name(header.name);
        self.headers.append(name, value);
        Ok(())
    }
}
