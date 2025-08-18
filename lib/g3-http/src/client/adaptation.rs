/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use http::{HeaderName, StatusCode, Version};
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use super::HttpResponseParseError;
use crate::{HttpHeaderLine, HttpLineParseError, HttpStatusLine};

#[derive(Debug)]
pub struct HttpAdaptedResponse {
    pub version: Version,
    pub status: StatusCode,
    pub reason: String,
    pub headers: HttpHeaderMap,
    pub content_length: Option<u64>,
}

impl HttpAdaptedResponse {
    fn new(version: Version, status: StatusCode, reason: String) -> Self {
        HttpAdaptedResponse {
            version,
            status,
            reason,
            headers: HttpHeaderMap::default(),
            content_length: None,
        }
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
            "connection" | "keep-alive" => {
                // ignored hop-by-hop options
                return Ok(());
            }
            "content-length" => {
                let content_length = u64::from_str(header.value)
                    .map_err(|_| HttpResponseParseError::InvalidContentLength)?;
                self.content_length = Some(content_length);
            }
            "transfer-encoding" => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;
    use tokio_test::io::Builder as MockIoBuilder;

    #[tokio::test]
    async fn parse_success() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-Custom: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert_eq!(rsp.version, Version::HTTP_11);
        assert_eq!(rsp.status, StatusCode::OK);
        assert_eq!(rsp.reason, "OK");
        assert_eq!(rsp.content_length, Some(5));
        assert_eq!(rsp.headers.get("x-custom").unwrap().to_str(), "value");
    }

    #[tokio::test]
    async fn http_10_version() {
        let data = b"HTTP/1.0 200 OK\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert_eq!(rsp.version, Version::HTTP_10);
        assert_eq!(rsp.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn http2_version() {
        let data = b"HTTP/2.0 200 OK\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert_eq!(rsp.version, Version::HTTP_2);
        assert_eq!(rsp.status, StatusCode::OK);
        assert_eq!(rsp.reason, "OK");
    }

    #[tokio::test]
    async fn invalid_status_line() {
        let data = b"INVALID/1.1 200 OK\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpResponseParseError::InvalidStatusLine(HttpLineParseError::InvalidVersion)
        ));
    }

    #[tokio::test]
    async fn ignore_hop_by_hop_headers() {
        let data = b"HTTP/1.1 200 OK\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5\r\nContent-Length: 0\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert!(rsp.headers.get("connection").is_none());
        assert!(rsp.headers.get("keep-alive").is_none());
    }

    #[tokio::test]
    async fn invalid_status_code() {
        let data = b"HTTP/1.1 99 Invalid\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpResponseParseError::InvalidStatusLine(HttpLineParseError::InvalidStatusCode)
        ));
    }

    #[tokio::test]
    async fn invalid_header_name() {
        let data = b"HTTP/1.1 200 OK\r\nInvalid@Header: value\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpResponseParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
        ));
    }

    #[tokio::test]
    async fn invalid_header_value() {
        let data = b"HTTP/1.1 200 OK\r\nX-Custom: \x00invalid\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            HttpResponseParseError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
        ));
    }

    #[tokio::test]
    async fn content_length_parsing() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 123\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert_eq!(rsp.content_length, Some(123));
    }

    #[tokio::test]
    async fn invalid_content_length() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: invalid\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpResponseParseError::InvalidContentLength));
    }

    #[tokio::test]
    async fn remote_closed() {
        let data = b"";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpResponseParseError::RemoteClosed));

        let data = b"HTTP/1.1 200 OK\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpResponseParseError::RemoteClosed));
    }

    #[tokio::test]
    async fn header_too_large() {
        let large_header = vec![b'A'; 1025];
        let mut data = b"HTTP/1.1 200 OK\r\n".to_vec();
        data.extend_from_slice(&large_header);
        data.extend_from_slice(b"\r\n\r\n");

        let mut reader = BufReader::new(MockIoBuilder::new().read(&data).build());
        let err = HttpAdaptedResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();

        assert!(matches!(err, HttpResponseParseError::TooLargeHeader(1024)));
    }

    #[tokio::test]
    async fn ignore_transfer_encoding() {
        let data = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        let mut reader = BufReader::new(MockIoBuilder::new().read(data).build());
        let rsp = HttpAdaptedResponse::parse(&mut reader, 1024).await.unwrap();

        assert!(!rsp.headers.contains_key("transfer-encoding"));
    }
}
