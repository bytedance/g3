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

use std::ops::Add;
use std::str::FromStr;
use std::time::Duration;

use tokio::io::AsyncBufRead;
use tokio::time::Instant;

use g3_io_ext::LimitedBufReadExt;

use super::IcapOptionsParseError;
use crate::parse::{HeaderLine, StatusLine};
use crate::IcapMethod;

pub struct IcapServiceOptions {
    method: IcapMethod,
    server: Option<String>,
    service_tag: String,
    service_id: Option<String>,
    max_connections: Option<usize>,
    expire: Option<Instant>,
    pub(crate) support_204: bool,
    pub(crate) support_206: bool,
    pub(crate) preview_size: Option<usize>,
}

impl IcapServiceOptions {
    pub(crate) fn new(method: IcapMethod) -> Self {
        IcapServiceOptions {
            method,
            server: None,
            service_tag: String::new(),
            service_id: None,
            max_connections: None,
            expire: None,
            support_204: false,
            support_206: false,
            preview_size: None,
        }
    }

    pub(crate) fn expired(&self) -> bool {
        if let Some(expire) = self.expire {
            Instant::now() > expire
        } else {
            false
        }
    }

    pub(crate) async fn parse<R>(
        reader: &mut R,
        method: IcapMethod,
        max_header_size: usize,
    ) -> Result<IcapServiceOptions, IcapOptionsParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut options = IcapServiceOptions::new(method);

        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut header_size = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', max_header_size, &mut line_buf)
            .await?;
        if nr == 0 {
            return Err(IcapOptionsParseError::RemoteClosed);
        }
        if !found {
            return if nr < max_header_size {
                Err(IcapOptionsParseError::RemoteClosed)
            } else {
                Err(IcapOptionsParseError::TooLargeHeader(max_header_size))
            };
        }
        header_size += nr;
        options.parse_status_line(&line_buf)?;

        loop {
            if header_size >= max_header_size {
                return Err(IcapOptionsParseError::TooLargeHeader(max_header_size));
            }
            line_buf.clear();
            let max_len = max_header_size - header_size;
            let (found, nr) = reader
                .limited_read_until(b'\n', max_len, &mut line_buf)
                .await?;
            if nr == 0 {
                return Err(IcapOptionsParseError::RemoteClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(IcapOptionsParseError::RemoteClosed)
                } else {
                    Err(IcapOptionsParseError::TooLargeHeader(max_header_size))
                };
            }
            header_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            options.parse_header_line(&line_buf)?;
        }
        options.check()?;

        Ok(options)
    }

    fn check(&self) -> Result<(), IcapOptionsParseError> {
        if self.service_tag.is_empty() {
            return Err(IcapOptionsParseError::NoServiceTagSet);
        }
        Ok(())
    }

    fn parse_status_line(&mut self, line: &[u8]) -> Result<(), IcapOptionsParseError> {
        let status = StatusLine::parse(line).map_err(IcapOptionsParseError::InvalidStatusLine)?;

        if status.code < 200 || status.code >= 300 {
            return Err(IcapOptionsParseError::RequestFailed(
                status.code,
                status.message.to_string(),
            ));
        }

        Ok(())
    }

    fn parse_header_line(&mut self, line: &[u8]) -> Result<(), IcapOptionsParseError> {
        let header = HeaderLine::parse(line).map_err(IcapOptionsParseError::InvalidHeaderLine)?;

        match header.name.to_lowercase().as_str() {
            "methods" => {
                if self.method.as_str() != header.value {
                    return Err(IcapOptionsParseError::MethodNotMatch);
                }
            }
            "service" => self.server = Some(header.value.to_string()),
            "istag" => self.service_tag = header.value.to_string(),
            "encapsulated" => {
                for p in header.value.split(',') {
                    let Some((name, _value)) = p.trim().split_once('=') else {
                        return Err(IcapOptionsParseError::InvalidHeaderValue("Encapsulated"));
                    };
                    match name.to_lowercase().as_str() {
                        "null-body" => {}
                        "opt-body" => {}
                        _ => return Err(IcapOptionsParseError::InvalidHeaderValue("Encapsulated")),
                    }
                }
            }
            "opt-body-type" => {
                return Err(IcapOptionsParseError::UnsupportedBody(
                    header.value.to_string(),
                ));
            }
            "max-connections" => {
                let max_connections = usize::from_str(header.value)
                    .map_err(|_| IcapOptionsParseError::InvalidHeaderValue("Max-Connections"))?;
                self.max_connections = Some(max_connections);
            }
            "options-ttl" => {
                let ttl = usize::from_str(header.value)
                    .map_err(|_| IcapOptionsParseError::InvalidHeaderValue("Options-TTL"))?;
                let expire = Instant::now().add(Duration::from_secs(ttl as u64));
                self.expire = Some(expire);
            }
            "service-id" => self.service_id = Some(header.value.to_string()),
            "allow" => {
                for p in header.value.split(',') {
                    let code = u16::from_str(p.trim())
                        .map_err(|_| IcapOptionsParseError::InvalidHeaderValue("Allow"))?;
                    match code {
                        204 => self.support_204 = true,
                        206 => self.support_206 = true,
                        _ => {}
                    }
                }
            }
            "preview" => {
                let size = usize::from_str(header.value)
                    .map_err(|_| IcapOptionsParseError::InvalidHeaderValue("Preview"))?;
                self.preview_size = Some(size);
            }
            _ => {}
        }

        Ok(())
    }
}
