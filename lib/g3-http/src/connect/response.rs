/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use http::HeaderName;
use tokio::io::AsyncBufRead;

use g3_io_ext::LimitedBufReadExt;
use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use super::{HttpConnectError, HttpConnectResponseError};
use crate::{HttpBodyReader, HttpBodyType, HttpHeaderLine, HttpLineParseError, HttpStatusLine};

#[derive(Debug)]
pub struct HttpConnectResponse {
    pub code: u16,
    pub reason: String,
    pub headers: HttpHeaderMap,
    content_length: u64,
    chunked_transfer: bool,
    has_transfer_encoding: bool,
    has_content_length: bool,
}

impl HttpConnectResponse {
    fn new(code: u16, reason: String) -> Self {
        HttpConnectResponse {
            code,
            reason,
            headers: HttpHeaderMap::default(),
            content_length: 0,
            chunked_transfer: false,
            has_transfer_encoding: false,
            has_content_length: false,
        }
    }

    fn body_type(&self) -> Option<HttpBodyType> {
        if self.chunked_transfer {
            Some(HttpBodyType::Chunked)
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

        rsp.post_check_and_fix();
        Ok(rsp)
    }

    /// do some necessary check and fix
    fn post_check_and_fix(&mut self) {
        // Don't move non-standard connection headers to hop-by-hop headers, as we don't support them
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

        let value = HttpHeaderValue::from_str(header.value).map_err(|_| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use http::header;
    use tokio::io::BufReader;
    use tokio_test::io::Builder;

    #[test]
    fn new() {
        let response = HttpConnectResponse::new(200, "OK".to_string());
        assert_eq!(response.code, 200);
        assert_eq!(response.reason, "OK");
        assert!(response.headers.is_empty());
        assert_eq!(response.content_length, 0);
        assert!(!response.chunked_transfer);
        assert!(!response.has_transfer_encoding);
        assert!(!response.has_content_length);
    }

    #[test]
    fn body_type() {
        // No body type
        let mut response = HttpConnectResponse::new(200, "OK".to_string());
        assert!(response.body_type().is_none());

        // Content length body type
        response.content_length = 100;
        response.has_content_length = true;
        assert_eq!(
            response.body_type().unwrap(),
            HttpBodyType::ContentLength(100)
        );

        // Chunked transfer body type
        response.content_length = 0;
        response.has_content_length = false;
        response.chunked_transfer = true;
        assert_eq!(response.body_type().unwrap(), HttpBodyType::Chunked);
    }

    #[test]
    fn build_from_status_line_valid() {
        // Valid status lines
        let cases = vec![
            (b"HTTP/1.0 200 OK\r\n".as_ref(), 200, "OK"),
            (b"HTTP/1.1 201 Created\r\n".as_ref(), 201, "Created"),
            (b"HTTP/2.0 404 Not Found\r\n".as_ref(), 404, "Not Found"),
            (b"HTTP/1.1 200 \r\n".as_ref(), 200, ""),
            (b"HTTP/1.1 200\r\n".as_ref(), 200, ""),
        ];

        for (input, expected_code, expected_reason) in cases {
            let result = HttpConnectResponse::build_from_status_line(input);
            let response = result.unwrap();
            assert_eq!(response.code, expected_code);
            assert_eq!(response.reason, expected_reason);
        }
    }

    #[test]
    fn build_from_status_line_invalid() {
        // Invalid status lines
        let cases = vec![
            b"HTTP/1.1".as_ref(),                    // Too short
            b"HTTP/3.0 200 OK\r\n".as_ref(),         // Invalid version
            b"HTTP/1.1 20 OK\r\n".as_ref(),          // Invalid status code
            b"HTTP/1.1 200 OK\xFF\xFF\r\n".as_ref(), // Invalid UTF-8
        ];

        for input in cases {
            let result = HttpConnectResponse::build_from_status_line(input);
            assert!(result.is_err());
        }
    }

    #[test]
    fn parse_header_line_valid() {
        let mut response = HttpConnectResponse::new(200, "OK".to_string());

        // Valid header lines
        let cases = vec![
            b"Content-Type: application/json\r\n".as_ref(),
            b"Content-Length: 100\r\n".as_ref(),
            b"Transfer-Encoding: chunked\r\n".as_ref(),
            b"Connection: keep-alive\r\n".as_ref(),
            b"  Accept  :  */*  \r\n".as_ref(),
        ];

        for input in cases {
            let result = response.parse_header_line(input);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn parse_header_line_invalid() {
        let mut response = HttpConnectResponse::new(200, "OK".to_string());

        // Invalid header lines
        let cases = vec![
            b"Invalid Header Without Colon\r\n".as_ref(),
            b"Content-Type application/json\r\n".as_ref(), // Missing colon
            b"Name\xFF: Value\r\n".as_ref(),               // Invalid UTF-8
        ];

        for input in cases {
            let result = response.parse_header_line(input);
            assert!(result.is_err());
        }
    }

    #[test]
    fn handle_header_content_length() {
        let mut response = HttpConnectResponse::new(200, "OK".to_string());

        // Content-length header
        let header = HttpHeaderLine {
            name: "content-length",
            value: "100",
        };

        let result = response.handle_header(header);
        assert!(result.is_ok());
        assert!(response.has_content_length);
        assert_eq!(response.content_length, 100);

        // Duplicate content-length with same value
        let header2 = HttpHeaderLine {
            name: "content-length",
            value: "100",
        };

        let result = response.handle_header(header2);
        assert!(result.is_ok());

        // Duplicate content-length with different value
        let header3 = HttpHeaderLine {
            name: "content-length",
            value: "200",
        };

        let result = response.handle_header(header3);
        assert!(result.is_err());
    }

    #[test]
    fn handle_header_transfer_encoding() {
        let mut response = HttpConnectResponse::new(200, "OK".to_string());

        // Transfer-encoding: chunked
        let header = HttpHeaderLine {
            name: "transfer-encoding",
            value: "chunked",
        };

        let result = response.handle_header(header);
        assert!(result.is_ok());
        assert!(response.has_transfer_encoding);
        assert!(response.chunked_transfer);

        // Invalid transfer-encoding with chunked in middle
        let header2 = HttpHeaderLine {
            name: "transfer-encoding",
            value: "gzip, chunked, deflate",
        };

        let result = response.handle_header(header2);
        assert!(result.is_err());

        // Transfer-encoding with chunked at end
        let header3 = HttpHeaderLine {
            name: "transfer-encoding",
            value: "gzip, deflate, chunked",
        };

        let result = response.handle_header(header3);
        assert!(result.is_ok());
        assert!(response.chunked_transfer);
    }

    #[test]
    fn handle_header_conflict() {
        // Add content-length then transfer-encoding
        let mut response = HttpConnectResponse::new(200, "OK".to_string());
        let content_length_header = HttpHeaderLine {
            name: "content-length",
            value: "100",
        };
        assert!(response.handle_header(content_length_header).is_ok());
        assert!(response.has_content_length);
        assert_eq!(response.content_length, 100);

        let transfer_encoding_header = HttpHeaderLine {
            name: "transfer-encoding",
            value: "chunked",
        };
        assert!(response.handle_header(transfer_encoding_header).is_ok());
        assert!(response.has_transfer_encoding);
        assert!(response.chunked_transfer);
        assert_eq!(response.content_length, 0);

        // Add transfer-encoding then content-length
        let mut response = HttpConnectResponse::new(200, "OK".to_string());
        let transfer_encoding_header = HttpHeaderLine {
            name: "transfer-encoding",
            value: "chunked",
        };
        assert!(response.handle_header(transfer_encoding_header).is_ok());
        assert!(response.has_transfer_encoding);

        let content_length_header = HttpHeaderLine {
            name: "content-length",
            value: "100",
        };
        assert!(response.handle_header(content_length_header).is_ok());
        assert!(!response.has_content_length);
        assert_eq!(response.content_length, 0);
    }

    #[test]
    fn handle_header_regular_header() {
        let mut response = HttpConnectResponse::new(200, "OK".to_string());

        // Regular header
        let header = HttpHeaderLine {
            name: "content-type",
            value: "application/json",
        };

        let result = response.handle_header(header);
        assert!(result.is_ok());
        assert_eq!(
            response.headers.get(header::CONTENT_TYPE).unwrap().to_str(),
            "application/json"
        );
    }

    #[test]
    fn detect_error() {
        // Successful status codes (200-299)
        for code in 200..=299 {
            let response = HttpConnectResponse::new(code, "OK".to_string());
            let result = response.detect_error();
            assert!(result.is_ok());
        }

        // Timeout status codes
        let timeout_codes = vec![504, 522, 524];
        for code in timeout_codes {
            let response = HttpConnectResponse::new(code, "Timeout".to_string());
            let err = response.detect_error().unwrap_err();
            assert!(matches!(err, HttpConnectError::PeerTimeout(_)));
        }

        // Other error status codes
        let error_codes = vec![400, 404, 500, 503];
        for code in error_codes {
            let response = HttpConnectResponse::new(code, "Error".to_string());
            let err = response.detect_error().unwrap_err();
            assert!(matches!(err, HttpConnectError::UnexpectedStatusCode(_, _)));
        }
    }

    #[tokio::test]
    async fn parse_successful_response() {
        let response_data =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let response = HttpConnectResponse::parse(&mut reader, 1024).await.unwrap();
        assert_eq!(response.code, 200);
        assert_eq!(response.reason, "OK");
        assert_eq!(
            response.headers.get(header::CONTENT_TYPE).unwrap().to_str(),
            "application/json"
        );
        assert_eq!(response.content_length, 100);
        assert!(response.has_content_length);
    }

    #[tokio::test]
    async fn parse_chunked_response() {
        let response_data = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let response = HttpConnectResponse::parse(&mut reader, 1024).await.unwrap();
        assert_eq!(response.code, 200);
        assert_eq!(response.reason, "OK");
        assert!(response.has_transfer_encoding);
        assert!(response.chunked_transfer);
        assert_eq!(response.content_length, 0);
    }

    #[tokio::test]
    async fn parse_remote_closed() {
        let stream = Builder::new().read(b"").build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::RemoteClosed));
    }

    #[tokio::test]
    async fn parse_too_large_header() {
        let response_data = b"HTTP/1.1 200 OK\r\nVery-Long-Header: ";
        let mut long_data = vec![0u8; 2048];
        long_data[..response_data.len()].copy_from_slice(response_data);
        let stream = Builder::new().read(&long_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::parse(&mut reader, 100)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn parse_invalid_status_line() {
        let response_data = b"INVALID STATUS LINE\r\nContent-Type: application/json\r\n\r\n";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn parse_invalid_header_line() {
        let response_data = b"HTTP/1.1 200 OK\r\nInvalid Header Without Colon\r\n\r\n";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::parse(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn recv_successful() {
        let response_data = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\nHello World!";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let response = HttpConnectResponse::recv(&mut reader, 1024).await.unwrap();
        assert_eq!(response.code, 200);
        assert_eq!(response.reason, "OK");
    }

    #[tokio::test]
    async fn recv_with_body() {
        let response_data = b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 9\r\n\r\nNot Found";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::recv(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            HttpConnectError::UnexpectedStatusCode(404, _)
        ));
    }

    #[tokio::test]
    async fn recv_chunked_response() {
        let response_data =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nHello\r\n0\r\n\r\n";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let response = HttpConnectResponse::recv(&mut reader, 1024).await.unwrap();
        assert_eq!(response.code, 200);
        assert_eq!(response.reason, "OK");
    }

    #[tokio::test]
    async fn recv_timeout_status() {
        let response_data = b"HTTP/1.1 504 Gateway Timeout\r\nContent-Type: text/plain\r\nContent-Length: 15\r\n\r\nGateway Timeout";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::recv(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::PeerTimeout(504)));
    }

    #[tokio::test]
    async fn recv_incomplete_body() {
        let response_data = b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nShort";
        let stream = Builder::new().read(response_data).build();
        let mut reader = BufReader::new(stream);

        let err = HttpConnectResponse::recv(&mut reader, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::ReadFailed(_)));
    }
}
