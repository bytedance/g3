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

use std::io::Write;
use std::str::FromStr;

use bytes::{BufMut, Bytes, BytesMut};
use http::{HeaderName, Method, Uri, Version};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue, UpstreamAddr};

use super::{HttpAdaptedRequest, HttpRequestParseError};
use crate::header::Connection;
use crate::{HttpBodyType, HttpHeaderLine, HttpLineParseError, HttpMethodLine};

pub struct HttpTransparentRequest {
    pub method: Method,
    pub version: Version,
    pub uri: Uri,
    pub end_to_end_headers: HttpHeaderMap,
    pub hop_by_hop_headers: HttpHeaderMap,
    /// the port may be 0
    pub host: Option<UpstreamAddr>,
    original_connection_name: Connection,
    extra_connection_headers: Vec<HeaderName>,
    origin_header_size: usize,
    keep_alive: bool,
    connection_upgrade: bool,
    pub upgrade: bool,
    content_length: u64,
    chunked_transfer: bool,
    chunked_with_trailer: bool,
    has_transfer_encoding: bool,
    has_content_length: bool,
    has_trailer: bool,
}

impl HttpTransparentRequest {
    fn new(method: Method, uri: Uri, version: Version) -> Self {
        HttpTransparentRequest {
            version,
            method,
            uri,
            end_to_end_headers: HttpHeaderMap::default(),
            hop_by_hop_headers: HttpHeaderMap::default(),
            host: None,
            original_connection_name: Connection::default(),
            extra_connection_headers: Vec::new(),
            origin_header_size: 0,
            keep_alive: false,
            connection_upgrade: false,
            upgrade: false,
            content_length: 0,
            chunked_transfer: false,
            chunked_with_trailer: false,
            has_transfer_encoding: false,
            has_content_length: false,
            has_trailer: false,
        }
    }

    pub fn clone_by_adaptation(&self, adapted: HttpAdaptedRequest) -> Self {
        let mut hop_by_hop_headers = self.hop_by_hop_headers.clone();
        hop_by_hop_headers.remove(http::header::TRAILER);
        for v in adapted.trailer() {
            hop_by_hop_headers.append(http::header::TRAILER, v.clone());
        }
        let chunked_with_trailer = !adapted.trailer().is_empty();
        HttpTransparentRequest {
            version: adapted.version,
            method: adapted.method,
            uri: adapted.uri,
            end_to_end_headers: adapted.headers,
            hop_by_hop_headers,
            host: None,
            original_connection_name: self.original_connection_name.clone(),
            extra_connection_headers: self.extra_connection_headers.clone(),
            origin_header_size: self.origin_header_size,
            keep_alive: self.keep_alive,
            connection_upgrade: self.connection_upgrade,
            upgrade: self.upgrade,
            content_length: self.content_length,
            chunked_transfer: true,
            chunked_with_trailer,
            has_transfer_encoding: false,
            has_content_length: false,
            has_trailer: false,
        }
    }

    #[inline]
    pub fn disable_keep_alive(&mut self) {
        self.keep_alive = false;
    }

    #[inline]
    pub fn keep_alive(&self) -> bool {
        self.keep_alive
    }

    pub fn body_type(&self) -> Option<HttpBodyType> {
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

    pub fn pipeline_safe(&self) -> bool {
        if matches!(
            &self.method,
            &Method::GET | &Method::HEAD | &Method::PUT | &Method::DELETE
        ) {
            if self.upgrade {
                return false;
            }
            // only pipeline idempotent requests without body
            if self.body_type().is_none() {
                // reader should not be sent
                return true;
            }
        }
        false
    }

    pub async fn parse<R>(
        reader: &mut R,
        max_header_size: usize,
    ) -> Result<(Self, Bytes), HttpRequestParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        let mut head_bytes = BytesMut::with_capacity(4096);

        let (found, nr) = reader
            .limited_read_buf_until(b'\n', max_header_size, &mut head_bytes)
            .await?;
        if nr == 0 {
            return Err(HttpRequestParseError::ClientClosed);
        }
        if !found {
            return if nr < max_header_size {
                Err(HttpRequestParseError::ClientClosed)
            } else {
                Err(HttpRequestParseError::TooLargeHeader(max_header_size))
            };
        }

        let mut req = HttpTransparentRequest::build_from_method_line(head_bytes.as_ref())?;
        match req.version {
            Version::HTTP_10 => req.keep_alive = false,
            Version::HTTP_11 => req.keep_alive = true,
            _ => {}
        }

        loop {
            let header_size = head_bytes.len();
            if header_size >= max_header_size {
                return Err(HttpRequestParseError::TooLargeHeader(max_header_size));
            }
            let max_len = max_header_size - header_size;
            let (found, nr) = reader
                .limited_read_buf_until(b'\n', max_len, &mut head_bytes)
                .await?;
            if nr == 0 {
                return Err(HttpRequestParseError::ClientClosed);
            }
            if !found {
                return if nr < max_len {
                    Err(HttpRequestParseError::ClientClosed)
                } else {
                    Err(HttpRequestParseError::TooLargeHeader(max_header_size))
                };
            }

            let line_buf = &head_bytes[header_size..];
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }
            req.parse_header_line(line_buf)?;
        }

        req.origin_header_size = head_bytes.len();
        req.post_check_and_fix();
        Ok((req, head_bytes.freeze()))
    }

    /// do some necessary check and fix
    fn post_check_and_fix(&mut self) {
        if !self.connection_upgrade {
            self.upgrade = false;
            self.hop_by_hop_headers.remove(http::header::UPGRADE);
        }
        if self.has_trailer && !self.chunked_transfer {
            self.hop_by_hop_headers.remove(http::header::TRAILER);
        }

        // Don't move non standard connection headers to hop-by-hop headers, as we don't support them
    }

    fn build_from_method_line(line_buf: &[u8]) -> Result<Self, HttpRequestParseError> {
        let req =
            HttpMethodLine::parse(line_buf).map_err(HttpRequestParseError::InvalidMethodLine)?;
        let version = match req.version {
            0 => Version::HTTP_10,
            1 => Version::HTTP_11,
            2 => return Err(HttpRequestParseError::UnsupportedVersion(Version::HTTP_2)),
            _ => unreachable!(),
        };

        let method = Method::from_str(req.method)
            .map_err(|_| HttpRequestParseError::UnsupportedMethod(req.method.to_string()))?;
        let uri =
            Uri::from_str(req.uri).map_err(|_| HttpRequestParseError::InvalidRequestTarget)?;
        Ok(HttpTransparentRequest::new(method, uri, version))
    }

    fn parse_header_line(&mut self, line_buf: &[u8]) -> Result<(), HttpRequestParseError> {
        let header =
            HttpHeaderLine::parse(line_buf).map_err(HttpRequestParseError::InvalidHeaderLine)?;
        self.handle_header(header)
    }

    pub fn parse_header_connection(
        &mut self,
        header: &HttpHeaderLine,
    ) -> Result<(), HttpRequestParseError> {
        let value = header.value.to_lowercase();

        for v in value.as_str().split(',') {
            if v.is_empty() {
                continue;
            }

            match v.trim() {
                "keep-alive" => {
                    self.keep_alive = true;
                }
                "close" => {
                    self.keep_alive = false;
                }
                "upgrade" => {
                    self.connection_upgrade = true;
                    self.extra_connection_headers.push(http::header::UPGRADE);
                }
                s => {
                    if let Ok(h) = HeaderName::from_str(s) {
                        self.extra_connection_headers.push(h);
                    }
                }
            }
        }

        self.original_connection_name = Connection::from_original(header.name);
        Ok(())
    }

    pub fn append_header(
        &mut self,
        name: HeaderName,
        header: &HttpHeaderLine,
    ) -> Result<(), HttpRequestParseError> {
        let mut value = HttpHeaderValue::from_str(header.value).map_err(|_| {
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        value.set_original_name(header.name);
        self.end_to_end_headers.append(name, value);
        Ok(())
    }

    fn insert_hop_by_hop_header(
        &mut self,
        name: HeaderName,
        header: &HttpHeaderLine,
    ) -> Result<(), HttpRequestParseError> {
        let mut value = HttpHeaderValue::from_str(header.value).map_err(|_| {
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        })?;
        value.set_original_name(header.name);
        self.hop_by_hop_headers.append(name, value);
        Ok(())
    }

    fn handle_header(&mut self, header: HttpHeaderLine) -> Result<(), HttpRequestParseError> {
        let name = HeaderName::from_str(header.name).map_err(|_| {
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        })?;

        match name.as_str() {
            "host" => {
                if self.host.is_some() {
                    return Err(HttpRequestParseError::InvalidHost);
                }
                if !header.value.is_empty() {
                    let host = UpstreamAddr::from_str(header.value)
                        .map_err(|_| HttpRequestParseError::InvalidHost)?;
                    // we didn't set the default port here, as we didn't know the server port
                    self.host = Some(host);
                }
            }
            "connection" => return self.parse_header_connection(&header),
            "upgrade" => {
                self.upgrade = true;
                return self.insert_hop_by_hop_header(name, &header);
            }
            "trailer" => {
                self.has_trailer = true;
                if self.chunked_transfer {
                    self.chunked_with_trailer = true;
                }
                return self.insert_hop_by_hop_header(name, &header);
            }
            "transfer-encoding" => {
                // it's a hop-by-hop option, but we just pass it
                self.has_transfer_encoding = true;
                if self.has_content_length {
                    // delete content-length
                    self.content_length = 0;
                }

                let v = header.value.to_lowercase();
                if v.ends_with("chunked") {
                    self.chunked_transfer = true;
                    if self.has_trailer {
                        self.chunked_with_trailer = true;
                    }
                } else {
                    return Err(HttpRequestParseError::InvalidChunkedTransferEncoding);
                }
                return self.insert_hop_by_hop_header(name, &header);
            }
            "content-length" => {
                if self.has_transfer_encoding {
                    // ignore content-length
                    return Ok(());
                }

                let content_length = u64::from_str(header.value)
                    .map_err(|_| HttpRequestParseError::InvalidContentLength)?;

                if self.has_content_length && self.content_length != content_length {
                    return Err(HttpRequestParseError::InvalidContentLength);
                }
                self.has_content_length = true;
                self.content_length = content_length;
            }
            "te" | "proxy-authorization" => {
                return self.insert_hop_by_hop_header(name, &header);
            }
            // ignore "expect"
            _ => {}
        }

        self.append_header(name, &header)
    }

    pub fn serialize_for_origin(&self) -> Vec<u8> {
        const RESERVED_LEN_FOR_EXTRA_HEADERS: usize = 256;
        let mut buf =
            Vec::<u8>::with_capacity(self.origin_header_size + RESERVED_LEN_FOR_EXTRA_HEADERS);
        if let Some(pa) = self.uri.path_and_query() {
            if self.method.eq(&Method::OPTIONS) && pa.query().is_none() && pa.path().eq("/") {
                let _ = write!(buf, "OPTIONS * {:?}\r\n", self.version);
            } else {
                let _ = write!(buf, "{} {} {:?}\r\n", self.method, pa, self.version);
            }
        } else if self.method.eq(&Method::OPTIONS) {
            let _ = write!(buf, "OPTIONS * {:?}\r\n", self.version);
        } else {
            let _ = write!(buf, "{} / {:?}\r\n", self.method, self.version);
        }
        self.end_to_end_headers
            .for_each(|name, value| value.write_to_buf(name, &mut buf));
        self.hop_by_hop_headers
            .for_each(|name, value| value.write_to_buf(name, &mut buf));
        self.original_connection_name.write_to_buf(
            !self.keep_alive,
            &self.extra_connection_headers,
            &mut buf,
        );
        buf.put_slice(b"\r\n");
        buf
    }

    pub fn serialize_for_adapter(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(self.origin_header_size);
        if let Some(pa) = self.uri.path_and_query() {
            if self.method.eq(&Method::OPTIONS) && pa.query().is_none() && pa.path().eq("/") {
                let _ = write!(buf, "OPTIONS * {:?}\r\n", self.version);
            } else {
                let _ = write!(buf, "{} {} {:?}\r\n", self.method, pa, self.version);
            }
        } else if self.method.eq(&Method::OPTIONS) {
            let _ = write!(buf, "OPTIONS * {:?}\r\n", self.version);
        } else {
            let _ = write!(buf, "{} / {:?}\r\n", self.method, self.version);
        }
        self.end_to_end_headers
            .for_each(|name, value| value.write_to_buf(name, &mut buf));
        buf.put_slice(b"\r\n");
        buf
    }
}

enum HttpTransparentRequestAcceptState {
    RecvMethodLine,
    RecvHeaderLine(HttpTransparentRequest),
    Finished(HttpTransparentRequest),
    End,
}

pub struct HttpTransparentRequestAcceptor {
    state: Option<HttpTransparentRequestAcceptState>,
}

impl Default for HttpTransparentRequestAcceptor {
    fn default() -> Self {
        HttpTransparentRequestAcceptor {
            state: Some(HttpTransparentRequestAcceptState::RecvMethodLine),
        }
    }
}

impl HttpTransparentRequestAcceptor {
    pub fn read_http(&mut self, buf: &[u8]) -> Result<usize, HttpRequestParseError> {
        let mut offset = 0;
        loop {
            match self.state.take() {
                Some(HttpTransparentRequestAcceptState::RecvMethodLine) => {
                    let Some(p) = memchr::memchr(b'\n', buf) else {
                        self.state = Some(HttpTransparentRequestAcceptState::RecvMethodLine);
                        return Ok(offset);
                    };

                    offset += p + 1;

                    let req = HttpTransparentRequest::build_from_method_line(&buf[0..=p])?;
                    self.state = Some(HttpTransparentRequestAcceptState::RecvHeaderLine(req));
                }
                Some(HttpTransparentRequestAcceptState::RecvHeaderLine(mut req)) => {
                    let Some(p) = memchr::memchr(b'\n', &buf[offset..]) else {
                        return Ok(offset);
                    };

                    let start = offset;
                    offset += p + 1;

                    let line_buf = &buf[start..offset];
                    if (line_buf.len() == 1 && line_buf[0] == b'\n')
                        || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
                    {
                        self.state = Some(HttpTransparentRequestAcceptState::Finished(req))
                    } else {
                        req.parse_header_line(line_buf)?;
                        self.state = Some(HttpTransparentRequestAcceptState::RecvHeaderLine(req))
                    }
                }
                Some(state) => {
                    self.state = Some(state);
                    return Ok(offset);
                }
                None => unreachable!(),
            }
        }
    }

    pub fn accept(&mut self) -> Option<HttpTransparentRequest> {
        let state = self.state.take();
        if let Some(HttpTransparentRequestAcceptState::Finished(req)) = state {
            self.state = Some(HttpTransparentRequestAcceptState::End);
            Some(req)
        } else {
            self.state = state;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http::Method;
    use tokio::io::{BufReader, Result};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn read_get() {
        let content = b"GET http://example.com/v/a/x HTTP/1.1\r\n\
            Host: example.com\r\n\
            Connection: Keep-Alive\r\n\
            Accept-Language: en-us,en;q=0.5\r\n\
            Accept-Encoding: gzip, deflate\r\n\
            Accept: */*\r\n\
            User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like G\
            ecko) Chrome/72.0.3611.2 Safari/537.36\r\n\
            Accept-Charset: ISO-8859-1,utf-8;q=0.7,*;q=0.7\r\n\r\n";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let (request, data) = HttpTransparentRequest::parse(&mut buf_stream, 4096)
            .await
            .unwrap();
        assert_eq!(data.as_ref(), content.as_slice());
        assert_eq!(request.method, &Method::GET);
        assert!(request.keep_alive());
        assert!(request.body_type().is_none());

        let result = HttpTransparentRequest::parse(&mut buf_stream, 4096).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connection_close() {
        let content = b"GET http://api.example.com/v1/files?api_key=abcd&ids=xyz HTTP/1.1\r\n\
            Accept: application/json, text/plain, */*\r\n\
            User-Agent: axios/0.21.1\r\n\
            host: api.giphy.com\r\n\
            Connection: close\r\n\r\n";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let (request, data) = HttpTransparentRequest::parse(&mut buf_stream, 4096)
            .await
            .unwrap();
        assert_eq!(data.as_ref(), content.as_slice());
        assert!(!request.keep_alive());
    }
}
