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

use http::{HeaderMap, HeaderName, HeaderValue};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;

use super::{HttpConnectError, HttpConnectResponseError};
use crate::{HttpBodyReader, HttpBodyType, HttpHeaderLine, HttpLineParseError, HttpStatusLine};

pub struct HttpConnectResponse {
    pub code: u16,
    pub reason: String,
    pub headers: HeaderMap,
    content_length: u64,
    chunked_transfer: bool,
    chunked_with_trailer: bool,
    has_transfer_encoding: bool,
    has_content_length: bool,
    has_trailer: bool,
}

impl HttpConnectResponse {
    fn new(code: u16, reason: String) -> Self {
        HttpConnectResponse {
            code,
            reason,
            headers: HeaderMap::new(),
            content_length: 0,
            chunked_transfer: false,
            chunked_with_trailer: false,
            has_transfer_encoding: false,
            has_content_length: false,
            has_trailer: false,
        }
    }

    fn body_type(&self) -> Option<HttpBodyType> {
        if self.chunked_transfer {
            if self.chunked_with_trailer {
                Some(HttpBodyType::ChunkedWithTrailer)
            } else {
                Some(HttpBodyType::ChunkedWithoutTrailer)
            }
        } else if self.content_length > 0 {
            Some(HttpBodyType::ContentLength(self.content_length))
        } else {
            None
        }
    }

    async fn parse<R>(reader: &mut R, max_header_size: usize) -> Result<Self, HttpConnectError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut header_size: usize = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', max_header_size, &mut line_buf)
            .await
            .map_err(HttpConnectError::ReadFailed)?;
        if nr == 0 {
            return Err(HttpConnectError::RemoteClosed);
        }
        if !found {
            return if nr < max_header_size {
                Err(HttpConnectError::RemoteClosed)
            } else {
                Err(HttpConnectResponseError::TooLargeHeader(max_header_size).into())
            };
        }
        header_size += nr;

        let mut rsp = HttpConnectResponse::build_from_status_line(line_buf.as_ref())?;

        loop {
            if header_size >= max_header_size {
                return Err(HttpConnectResponseError::TooLargeHeader(max_header_size).into());
            }
            line_buf.clear();
            let max_len = max_header_size - header_size;
            let (found, nr) = reader
                .limited_read_until(b'\n', max_len, &mut line_buf)
                .await
                .map_err(HttpConnectError::ReadFailed)?;
            if nr == 0 {
                return Err(HttpConnectError::RemoteClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(HttpConnectError::RemoteClosed)
                } else {
                    Err(HttpConnectResponseError::TooLargeHeader(max_header_size).into())
                };
            }
            header_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            rsp.parse_header_line(line_buf.as_ref())?;
        }

        Ok(rsp)
    }

    fn build_from_status_line(line_buf: &[u8]) -> Result<Self, HttpConnectResponseError> {
        let rsp =
            HttpStatusLine::parse(line_buf).map_err(HttpConnectResponseError::InvalidStatusLine)?;
        Ok(HttpConnectResponse::new(rsp.code, rsp.reason.to_string()))
    }

    fn parse_header_line(&mut self, line_buf: &[u8]) -> Result<(), HttpConnectResponseError> {
        let header =
            HttpHeaderLine::parse(line_buf).map_err(HttpConnectResponseError::InvalidHeaderLine)?;
        self.handle_header(header)
    }

    fn handle_header(&mut self, header: HttpHeaderLine) -> Result<(), HttpConnectResponseError> {
        let name = HeaderName::from_str(header.name).map_err(|_| {
            HttpConnectResponseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        })?;

        match name.as_str() {
            "connection" | "proxy-connection" => {}
            "trailer" => {
                self.has_trailer = true;
                if self.chunked_transfer {
                    self.chunked_with_trailer = true;
                }
            }
            "transfer-encoding" => {
                self.has_transfer_encoding = true;
                if self.has_content_length {
                    // delete content-length
                    self.headers.remove(http::header::CONTENT_LENGTH);
                    self.content_length = 0;
                }

                let v = header.value.to_lowercase();
                if v.ends_with("chunked") {
                    self.chunked_transfer = true;
                    if self.has_trailer {
                        self.chunked_with_trailer = true;
                    }
                } else if v.contains("chunked") {
                    return Err(HttpConnectResponseError::InvalidChunkedTransferEncoding);
                }
            }
            "content-length" => {
                if self.has_transfer_encoding {
                    // ignore content-length
                    return Ok(());
                }

                let content_length = u64::from_str(header.value)
                    .map_err(|_| HttpConnectResponseError::InvalidContentLength)?;

                if self.has_content_length && self.content_length != content_length {
                    return Err(HttpConnectResponseError::InvalidContentLength);
                }
                self.has_content_length = true;
                self.content_length = content_length;
            }
            _ => {}
        }

        let value = HeaderValue::from_str(header.value).map_err(|_| {
            HttpConnectResponseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        self.headers.append(name, value);
        Ok(())
    }

    fn detect_error(&self) -> Result<(), HttpConnectError> {
        if self.code >= 200 && self.code < 300 {
            Ok(())
        } else if self.code == 504 || self.code == 522 || self.code == 524 {
            // Peer tells us it timeout
            Err(HttpConnectError::PeerTimeout(self.code))
        } else {
            Err(HttpConnectError::UnexpectedStatusCode(
                self.code,
                self.reason.to_string(),
            ))
        }
    }

    pub async fn recv<R>(r: &mut R, max_header_size: usize) -> Result<Self, HttpConnectError>
    where
        R: AsyncBufRead + Unpin,
    {
        let rsp = HttpConnectResponse::parse(r, max_header_size).await?;

        if let Some(body_type) = rsp.body_type() {
            // the body should be simple in non-2xx case, use a default 2048 for its max line size
            let mut body_reader = HttpBodyReader::new(r, body_type, 2048);
            let mut sink = tokio::io::sink();
            tokio::io::copy(&mut body_reader, &mut sink)
                .await
                .map_err(HttpConnectError::ReadFailed)?;
        }

        rsp.detect_error()?;

        Ok(rsp)
    }
}
