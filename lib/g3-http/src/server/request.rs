/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashSet;
use std::io::Write;
use std::str::FromStr;

use bytes::BufMut;
use http::{HeaderName, Method, Uri, Version, header};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{Host, HttpAuth, HttpHeaderMap, HttpHeaderValue, UpstreamAddr};

use super::{HttpAdaptedRequest, HttpRequestParseError};
use crate::header::Connection;
use crate::{HttpBodyType, HttpHeaderLine, HttpLineParseError, HttpMethodLine};

pub struct HttpProxyClientRequest {
    pub version: Version,
    pub method: Method,
    pub uri: Uri,
    pub end_to_end_headers: HttpHeaderMap,
    pub hop_by_hop_headers: HttpHeaderMap,
    pub auth_info: HttpAuth,
    /// the port may be 0
    pub host: Option<UpstreamAddr>,
    original_connection_name: Connection,
    extra_connection_headers: Vec<HeaderName>,
    origin_header_size: usize,
    keep_alive: bool,
    content_length: u64,
    chunked_transfer: bool,
    has_transfer_encoding: bool,
    has_content_length: bool,
}

impl HttpProxyClientRequest {
    fn new(method: Method, uri: Uri, version: Version) -> Self {
        HttpProxyClientRequest {
            version,
            method,
            uri,
            end_to_end_headers: HttpHeaderMap::default(),
            hop_by_hop_headers: HttpHeaderMap::default(),
            auth_info: HttpAuth::None,
            host: None,
            original_connection_name: Connection::default(),
            extra_connection_headers: Vec::new(),
            origin_header_size: 0,
            keep_alive: false,
            content_length: 0,
            chunked_transfer: false,
            has_transfer_encoding: false,
            has_content_length: false,
        }
    }

    pub fn adapt_with_body(&self, adapted: HttpAdaptedRequest) -> Self {
        let mut hop_by_hop_headers = self.hop_by_hop_headers.clone();
        match adapted.content_length {
            Some(content_length) => {
                hop_by_hop_headers.remove(header::TRANSFER_ENCODING);
                HttpProxyClientRequest {
                    version: adapted.version,
                    method: adapted.method,
                    uri: adapted.uri,
                    end_to_end_headers: adapted.headers,
                    hop_by_hop_headers,
                    auth_info: HttpAuth::None,
                    host: None,
                    original_connection_name: self.original_connection_name.clone(),
                    extra_connection_headers: self.extra_connection_headers.clone(),
                    origin_header_size: self.origin_header_size,
                    keep_alive: self.keep_alive,
                    content_length,
                    chunked_transfer: false,
                    has_transfer_encoding: false,
                    has_content_length: true,
                }
            }
            None => {
                if !self.chunked_transfer {
                    if let Some(mut v) = hop_by_hop_headers.remove(header::TRANSFER_ENCODING) {
                        v.set_static_value("chunked");
                        hop_by_hop_headers.insert(header::TRANSFER_ENCODING, v);
                    } else {
                        hop_by_hop_headers.insert(
                            header::TRANSFER_ENCODING,
                            HttpHeaderValue::from_static("chunked"),
                        );
                    }
                }
                HttpProxyClientRequest {
                    version: adapted.version,
                    method: adapted.method,
                    uri: adapted.uri,
                    end_to_end_headers: adapted.headers,
                    hop_by_hop_headers,
                    auth_info: HttpAuth::None,
                    host: None,
                    original_connection_name: self.original_connection_name.clone(),
                    extra_connection_headers: self.extra_connection_headers.clone(),
                    origin_header_size: self.origin_header_size,
                    keep_alive: self.keep_alive,
                    content_length: 0,
                    chunked_transfer: true,
                    has_transfer_encoding: true,
                    has_content_length: false,
                }
            }
        }
    }

    pub fn adapt_without_body(&self, adapted: HttpAdaptedRequest) -> Self {
        let mut hop_by_hop_headers = self.hop_by_hop_headers.clone();
        hop_by_hop_headers.remove(header::TRANSFER_ENCODING);
        HttpProxyClientRequest {
            version: adapted.version,
            method: adapted.method,
            uri: adapted.uri,
            end_to_end_headers: adapted.headers,
            hop_by_hop_headers,
            auth_info: HttpAuth::None,
            host: None,
            original_connection_name: self.original_connection_name.clone(),
            extra_connection_headers: self.extra_connection_headers.clone(),
            origin_header_size: self.origin_header_size,
            keep_alive: self.keep_alive,
            content_length: 0,
            chunked_transfer: false,
            has_transfer_encoding: false,
            has_content_length: false,
        }
    }

    #[inline]
    pub fn origin_header_size(&self) -> usize {
        self.origin_header_size
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
            Some(HttpBodyType::Chunked)
        } else if self.content_length > 0 {
            Some(HttpBodyType::ContentLength(self.content_length))
        } else {
            None
        }
    }

    pub fn has_auth_info(&self) -> bool {
        !matches!(self.auth_info, HttpAuth::None)
    }

    pub fn pipeline_safe(&self) -> bool {
        if matches!(
            &self.method,
            &Method::GET | &Method::HEAD | &Method::PUT | &Method::DELETE
        ) {
            // only pipeline idempotent requests without body
            if self.body_type().is_none() {
                // reader should not be sent
                return true;
            }
        }
        false
    }

    pub fn is_local_request(&self, local_names: &HashSet<Host>) -> bool {
        if local_names.is_empty() {
            // no local server name set, treat request without absolute URI as local targeted
            self.uri.scheme().is_none()
        } else {
            self.host
                .as_ref()
                .map(|addr| local_names.contains(addr.host()))
                .unwrap_or(false)
        }
    }

    pub fn set_host(&mut self, host: &UpstreamAddr) {
        let mut new_v = unsafe { HttpHeaderValue::from_string_unchecked(host.to_string()) };
        if let Some(old_v) = self.end_to_end_headers.remove(header::HOST) {
            if let Some(name) = old_v.original_name() {
                new_v.set_original_name(name);
            }
        }
        self.end_to_end_headers.insert(header::HOST, new_v);
        self.host = Some(host.clone());
    }

    pub async fn parse_basic<R>(
        reader: &mut R,
        max_header_size: usize,
        version: &mut Version,
    ) -> Result<Self, HttpRequestParseError>
    where
        R: AsyncBufRead + Unpin,
    {
        Self::parse(reader, max_header_size, version, |req, name, value| {
            req.append_parsed_header(name, value)
        })
        .await
    }

    pub async fn parse<R, F>(
        reader: &mut R,
        max_header_size: usize,
        version: &mut Version,
        parse_more_header: F,
    ) -> Result<Self, HttpRequestParseError>
    where
        R: AsyncBufRead + Unpin,
        F: Fn(&mut Self, HeaderName, &HttpHeaderLine) -> Result<(), HttpRequestParseError>,
    {
        let mut line_buf = Vec::<u8>::with_capacity(1024);
        let mut header_size: usize = 0;

        let (found, nr) = reader
            .limited_read_until(b'\n', max_header_size, &mut line_buf)
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
        header_size += nr;

        let mut req = HttpProxyClientRequest::build_from_method_line(line_buf.as_ref())?;
        match req.version {
            Version::HTTP_10 => req.keep_alive = false,
            Version::HTTP_11 => req.keep_alive = true,
            _ => unreachable!(),
        }
        *version = req.version; // always set version in case of error

        loop {
            if header_size >= max_header_size {
                return Err(HttpRequestParseError::TooLargeHeader(max_header_size));
            }
            line_buf.clear();
            let max_len = max_header_size - header_size;
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
                    Err(HttpRequestParseError::TooLargeHeader(max_header_size))
                };
            }
            header_size += nr;
            if (line_buf.len() == 1 && line_buf[0] == b'\n')
                || (line_buf.len() == 2 && line_buf[0] == b'\r' && line_buf[1] == b'\n')
            {
                // header end line
                break;
            }

            req.parse_header_line(line_buf.as_ref(), &parse_more_header)?;
        }
        req.origin_header_size = header_size;

        req.post_check_and_fix();
        Ok(req)
    }

    /// do some necessary check and fix
    fn post_check_and_fix(&mut self) {
        // Don't move non-standard connection headers to hop-by-hop headers, as we don't support them
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
        Ok(HttpProxyClientRequest::new(method, uri, version))
    }

    fn parse_header_line<F>(
        &mut self,
        line_buf: &[u8],
        parse_more_header: &F,
    ) -> Result<(), HttpRequestParseError>
    where
        F: Fn(&mut Self, HeaderName, &HttpHeaderLine) -> Result<(), HttpRequestParseError>,
    {
        let header =
            HttpHeaderLine::parse(line_buf).map_err(HttpRequestParseError::InvalidHeaderLine)?;
        self.handle_header(header, parse_more_header)
    }

    pub fn parse_header_authorization(&mut self, value: &str) -> Result<(), HttpRequestParseError> {
        self.auth_info = HttpAuth::from_authorization(value)
            .map_err(|_| HttpRequestParseError::UnsupportedAuthorization)?;
        Ok(())
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

    pub fn append_parsed_header(
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

    fn handle_header<F>(
        &mut self,
        header: HttpHeaderLine,
        parse_more_header: &F,
    ) -> Result<(), HttpRequestParseError>
    where
        F: Fn(&mut Self, HeaderName, &HttpHeaderLine) -> Result<(), HttpRequestParseError>,
    {
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
            "keep-alive" => {
                // the client should not send this, just ignore it
                return Ok(());
            }
            "te" => {
                // hop-by-hop option, but let's pass it
                return self.insert_hop_by_hop_header(name, &header);
            }
            "upgrade" => {
                // TODO we have no support for it right now
                return Err(HttpRequestParseError::UpgradeIsNotSupported);
            }
            "transfer-encoding" => {
                // it's a hop-by-hop option, but we just pass it
                self.has_transfer_encoding = true;
                if self.has_content_length {
                    // delete content-length
                    self.end_to_end_headers.remove(header::CONTENT_LENGTH);
                    self.content_length = 0;
                    self.keep_alive = false; // according to rfc9112 Section 6.1
                }

                let v = header.value.to_lowercase();
                if v.ends_with("chunked") {
                    self.chunked_transfer = true;
                } else {
                    return Err(HttpRequestParseError::InvalidChunkedTransferEncoding);
                }
                return self.insert_hop_by_hop_header(name, &header);
            }
            "content-length" => {
                if self.has_transfer_encoding {
                    // ignore content-length
                    self.keep_alive = false; // according to rfc9112 Section 6.1
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
            // ignore "expect"
            _ => {}
        }

        parse_more_header(self, name, &header)
    }

    pub fn serialize_for_origin(&self) -> Vec<u8> {
        const RESERVED_LEN_FOR_EXTRA_HEADERS: usize = 256;
        let mut buf =
            Vec::<u8>::with_capacity(self.origin_header_size + RESERVED_LEN_FOR_EXTRA_HEADERS);
        if let Some(pa) = self.uri.path_and_query() {
            let _ = write!(buf, "{} {} {:?}\r\n", self.method, pa, self.version);
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

    pub fn partial_serialize_for_proxy(
        &self,
        upstream: &UpstreamAddr,
        reserve_size: usize,
    ) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(self.origin_header_size + reserve_size);
        let scheme = self.uri.scheme_str().unwrap_or("http");
        if let Some(pa) = self.uri.path_and_query() {
            let _ = write!(
                buf,
                "{} {}://{}{} {:?}\r\n",
                self.method, scheme, upstream, pa, self.version
            );
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
        buf
    }

    pub fn serialize_for_adapter(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(self.origin_header_size);
        if let Some(pa) = self.uri.path_and_query() {
            let _ = write!(buf, "{} {} {:?}\r\n", self.method, pa, self.version);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    fn parse_more_header(
        req: &mut HttpProxyClientRequest,
        name: HeaderName,
        value: &HttpHeaderLine,
    ) -> Result<(), HttpRequestParseError> {
        req.append_parsed_header(name, value)?;
        Ok(())
    }

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
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut version = Version::HTTP_11;
        let request =
            HttpProxyClientRequest::parse(&mut buf_stream, 4096, &mut version, parse_more_header)
                .await
                .unwrap();
        assert_eq!(request.method, &Method::GET);
        assert!(request.keep_alive());
        assert!(request.body_type().is_none());

        let result =
            HttpProxyClientRequest::parse(&mut buf_stream, 4096, &mut version, parse_more_header)
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connection_close() {
        let content = b"GET http://api.example.com/v1/files?api_key=abcd&ids=xyz HTTP/1.1\r\n\
            Accept: application/json, text/plain, */*\r\n\
            User-Agent: axios/0.21.1\r\n\
            host: api.giphy.com\r\n\
            Connection: close\r\n\r\n";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut version = Version::HTTP_11;
        let request =
            HttpProxyClientRequest::parse(&mut buf_stream, 4096, &mut version, parse_more_header)
                .await
                .unwrap();
        assert!(!request.keep_alive());
    }
}
