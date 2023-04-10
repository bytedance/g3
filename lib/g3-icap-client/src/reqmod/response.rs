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

use std::collections::BTreeSet;
use std::str::FromStr;

use http::HeaderName;
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use super::{IcapReqmodParseError, IcapReqmodResponsePayload};
use crate::parse::{HeaderLine, IcapLineParseError, StatusLine};

pub(crate) struct ReqmodResponse {
    pub(crate) code: u16,
    pub(crate) reason: String,
    pub(crate) keep_alive: bool,
    pub(crate) payload: IcapReqmodResponsePayload,
    shared_headers: HttpHeaderMap,
    trailers: Vec<HttpHeaderValue>,
}

impl ReqmodResponse {
    fn new(code: u16, reason: String) -> Self {
        ReqmodResponse {
            code,
            reason,
            keep_alive: true,
            payload: IcapReqmodResponsePayload::NoPayload,
            shared_headers: HttpHeaderMap::default(),
            trailers: Vec::new(),
        }
    }

    pub(crate) fn take_trailers(&mut self) -> Vec<HttpHeaderValue> {
        self.trailers.drain(..).collect()
    }

    pub(crate) fn take_shared_headers(&mut self) -> HttpHeaderMap {
        std::mem::take(&mut self.shared_headers)
    }

    pub(crate) async fn parse<R>(
        reader: &mut R,
        max_header_size: usize,
        shared_names: &BTreeSet<String>,
    ) -> Result<ReqmodResponse, IcapReqmodParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut header_size = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', max_header_size, &mut line_buf)
            .await?;
        if nr == 0 {
            return Err(IcapReqmodParseError::RemoteClosed);
        }
        if !found {
            return if nr < max_header_size {
                Err(IcapReqmodParseError::RemoteClosed)
            } else {
                Err(IcapReqmodParseError::TooLargeHeader(max_header_size))
            };
        }
        header_size += nr;
        let mut rsp = Self::build_from_status_line(&line_buf)?;

        loop {
            if header_size >= max_header_size {
                return Err(IcapReqmodParseError::TooLargeHeader(max_header_size));
            }
            line_buf.clear();
            let max_len = max_header_size - header_size;
            let (found, nr) = reader
                .limited_read_until(b'\n', max_len, &mut line_buf)
                .await?;
            if nr == 0 {
                return Err(IcapReqmodParseError::RemoteClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(IcapReqmodParseError::RemoteClosed)
                } else {
                    Err(IcapReqmodParseError::TooLargeHeader(max_header_size))
                };
            }
            header_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            rsp.parse_header_line(&line_buf, shared_names)?;
        }

        Ok(rsp)
    }

    fn build_from_status_line(line_buf: &[u8]) -> Result<Self, IcapReqmodParseError> {
        let status =
            StatusLine::parse(line_buf).map_err(IcapReqmodParseError::InvalidStatusLine)?;

        let rsp = ReqmodResponse::new(status.code, status.message.to_string());
        Ok(rsp)
    }

    fn parse_header_line(
        &mut self,
        line: &[u8],
        shared_names: &BTreeSet<String>,
    ) -> Result<(), IcapReqmodParseError> {
        let header = HeaderLine::parse(line).map_err(IcapReqmodParseError::InvalidHeaderLine)?;

        match header.name.to_lowercase().as_str() {
            "connection" => {
                let value = header.value.to_lowercase();

                for v in value.as_str().split(',') {
                    if v.is_empty() {
                        continue;
                    }

                    match v.trim() {
                        "keep-alive" => {
                            // keep the original value from request
                        }
                        "close" => {
                            self.keep_alive = false;
                        }
                        _ => {} // ignore other custom hop-by-hop headers
                    }
                }
            }
            "trailer" => {
                let value = HttpHeaderValue::from_str(header.value).map_err(|_| {
                    IcapReqmodParseError::InvalidHeaderLine(IcapLineParseError::InvalidTrailerValue)
                })?;
                self.trailers.push(value);
            }
            "encapsulated" => self.payload = IcapReqmodResponsePayload::parse(header.value)?,
            header_name => {
                if shared_names.contains(header_name) {
                    let name = HeaderName::from_str(header_name).map_err(|_| {
                        IcapReqmodParseError::InvalidHeaderLine(
                            IcapLineParseError::InvalidHeaderName,
                        )
                    })?;
                    let value = HttpHeaderValue::from_str(header.value).map_err(|_| {
                        IcapReqmodParseError::InvalidHeaderLine(
                            IcapLineParseError::InvalidHeaderValue,
                        )
                    })?;
                    self.shared_headers.append(name, value);
                }
            }
        }

        Ok(())
    }
}
