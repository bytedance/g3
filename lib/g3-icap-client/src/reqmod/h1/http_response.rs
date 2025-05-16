/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::Write;
use std::str::FromStr;

use bytes::BufMut;
use http::{HeaderName, StatusCode, Version};
use tokio::io::AsyncBufRead;

use g3_http::client::HttpResponseParseError;
use g3_http::{HttpHeaderLine, HttpLineParseError, HttpStatusLine};
use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

pub struct HttpAdapterErrorResponse {
    pub version: Version,
    pub status: StatusCode,
    pub reason: String,
    pub headers: HttpHeaderMap,
}

impl HttpAdapterErrorResponse {
    fn new(version: Version, status: StatusCode, reason: String) -> Self {
        HttpAdapterErrorResponse {
            version,
            status,
            reason,
            headers: HttpHeaderMap::default(),
        }
    }

    pub(crate) fn set_chunked_encoding(&mut self) {
        self.headers.insert(
            http::header::TRANSFER_ENCODING,
            HttpHeaderValue::from_static("chunked"),
        );
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

        let mut rsp = HttpAdapterErrorResponse::build_from_status_line(&line_buf)?;

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

        Ok(HttpAdapterErrorResponse::new(
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
            "connection" | "keep-alive" => return Ok(()),
            "transfer-encoding" | "content-length" => return Ok(()),
            _ => {}
        }

        let mut value = HttpHeaderValue::from_str(header.value).map_err(|_| {
            HttpResponseParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        value.set_original_name(header.name);
        self.headers.append(name, value);
        Ok(())
    }

    pub fn serialize(&self, close_connection: bool) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(2048);

        let _ = write!(
            buf,
            "{:?} {} {}\r\n",
            self.version,
            self.status.as_u16(),
            self.reason
        );

        self.headers
            .for_each(|name, value| value.write_to_buf(name, &mut buf));
        let connection_value = g3_http::header::connection_as_bytes(close_connection);
        buf.put_slice(connection_value);
        buf.put_slice(b"\r\n");
        buf
    }
}
