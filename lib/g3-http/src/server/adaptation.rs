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

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;
    use tokio_test::io::Builder as MockIoBuilder;

    #[tokio::test]
    async fn parse_success() {
        // Successful parsing of HTTP/1.1 request
        let data = b"GET /index.html HTTP/1.1\r\nContent-Length: 5\r\nX-Custom: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert_eq!(req.method, Method::GET);
        assert_eq!(req.uri, "/index.html");
        assert_eq!(req.version, Version::HTTP_11);
        assert_eq!(req.content_length, Some(5));
        assert_eq!(req.headers.get("x-custom").unwrap().to_str(), "value");
    }

    #[tokio::test]
    async fn http_10_version() {
        // HTTP/1.0 version
        let data = b"GET / HTTP/1.0\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert_eq!(req.version, Version::HTTP_10);
        assert_eq!(req.method, Method::GET);
        assert_eq!(req.uri, "/");
    }

    #[tokio::test]
    async fn http2_version() {
        // HTTP/2 version
        let data = b"POST /api HTTP/2\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert_eq!(req.version, Version::HTTP_2);
        assert_eq!(req.method, Method::POST);
        assert_eq!(req.uri, "/api");
    }

    #[tokio::test]
    async fn invalid_method_line() {
        // Invalid method line (missing space)
        let data = b"GET/index.html HTTP/1.1\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpRequestParseError::InvalidMethodLine(HttpLineParseError::NoDelimiterFound(' '))
        ));
    }

    #[tokio::test]
    async fn unsupported_method() {
        // Unsupported HTTP method - use a method with invalid characters
        let data = b"GET@INVALID / HTTP/1.1\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::UnsupportedMethod(_)));
    }

    #[tokio::test]
    async fn invalid_request_target() {
        // Invalid request target - use a URI with invalid characters
        let data = b"GET http://example.com/\x00 HTTP/1.1\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::InvalidRequestTarget));
    }

    #[tokio::test]
    async fn client_closed() {
        // Client closed connection
        let data = b"";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::ClientClosed));

        // Client closed during header reading
        let data = b"GET / HTTP/1.1\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::ClientClosed));
    }

    #[tokio::test]
    async fn header_too_large() {
        // Header size exceeding limit
        let large_header = vec![b'A'; 1025];
        let mut data = b"GET / HTTP/1.1\r\n".to_vec();
        data.extend_from_slice(&large_header);
        data.extend_from_slice(b"\r\n\r\n");

        let mut reader = BufReader::new(MockIoBuilder::new().read(&data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::TooLargeHeader(1024)));
    }

    #[tokio::test]
    async fn invalid_header_name() {
        // Invalid header name
        let data = b"GET / HTTP/1.1\r\nInvalid@Header: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        ));
    }

    #[tokio::test]
    async fn invalid_header_value() {
        // Invalid header value
        let data = b"GET / HTTP/1.1\r\nX-Custom: \x00invalid\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpRequestParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        ));
    }

    #[tokio::test]
    async fn content_length_parsing() {
        // Content-length header parsing
        let data = b"POST /upload HTTP/1.1\r\nContent-Length: 123\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert_eq!(req.content_length, Some(123));
    }

    #[tokio::test]
    async fn invalid_content_length() {
        // Invalid content-length value
        let data = b"POST /upload HTTP/1.1\r\nContent-Length: invalid\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpRequestParseError::InvalidContentLength));
    }

    #[tokio::test]
    async fn ignore_hop_by_hop_headers() {
        // Hop-by-hop headers are ignored
        let data = b"GET / HTTP/1.1\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5\r\nTE: trailers\r\nContent-Length: 0\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert!(req.headers.get("connection").is_none());
        assert!(req.headers.get("keep-alive").is_none());
        assert!(req.headers.get("te").is_none());
        assert_eq!(req.content_length, Some(0));
    }

    #[tokio::test]
    async fn ignore_transfer_encoding() {
        // Transfer-encoding header is ignored
        let data =
            b"POST /upload HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert!(!req.headers.contains_key("transfer-encoding"));
        assert_eq!(req.content_length, Some(5));
    }

    #[tokio::test]
    async fn via_header_with_ignore_via_false() {
        // Via header when ignore_via is false
        let data = b"GET / HTTP/1.1\r\nVia: 1.1 proxy.example.com\r\nX-Custom: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert!(req.headers.contains_key("via"));
        assert_eq!(
            req.headers.get("via").unwrap().to_str(),
            "1.1 proxy.example.com"
        );
        assert_eq!(req.headers.get("x-custom").unwrap().to_str(), "value");
    }

    #[tokio::test]
    async fn via_header_with_ignore_via_true() {
        // Via header when ignore_via is true
        let data = b"GET / HTTP/1.1\r\nVia: 1.1 proxy.example.com\r\nX-Custom: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, true)
            .await
            .unwrap();

        assert!(!req.headers.contains_key("via"));
        assert_eq!(req.headers.get("x-custom").unwrap().to_str(), "value");
    }

    #[tokio::test]
    async fn multiple_headers() {
        // Multiple headers with same name
        let data = b"GET / HTTP/1.1\r\nX-Custom: value1\r\nX-Custom: value2\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        let values: Vec<_> = req
            .headers
            .get_all("x-custom")
            .iter()
            .map(|v| v.to_str())
            .collect();
        assert_eq!(values, vec!["value1", "value2"]);
    }

    #[tokio::test]
    async fn header_with_whitespace() {
        // Headers with surrounding whitespace
        let data = b"GET / HTTP/1.1\r\n  X-Custom  :  value with spaces  \r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let req = HttpAdaptedRequest::parse(&mut reader, 1024, false)
            .await
            .unwrap();

        assert_eq!(
            req.headers.get("x-custom").unwrap().to_str(),
            "value with spaces"
        );
    }
}
