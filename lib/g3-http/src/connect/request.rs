/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use g3_types::net::UpstreamAddr;

/// the extra header lines should end with \r\n
pub struct HttpConnectRequest<'a> {
    host: &'a UpstreamAddr,
    static_headers: &'a [String],
    dyn_headers: Vec<String>,
}

impl<'a> HttpConnectRequest<'a> {
    pub fn new(host: &'a UpstreamAddr, static_headers: &'a [String]) -> Self {
        HttpConnectRequest {
            host,
            static_headers,
            dyn_headers: Vec::new(),
        }
    }

    pub fn append_dyn_header(&mut self, line: String) {
        debug_assert!(line.ends_with("\r\n"));
        self.dyn_headers.push(line);
    }

    /// the extra header lines should end with \r\n
    pub async fn send<W>(&'a self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf_writer = BufWriter::new(writer);
        buf_writer
            .write_all(format!("CONNECT {} HTTP/1.1\r\n", self.host).as_bytes())
            .await?;
        buf_writer
            .write_all(format!("Host: {}\r\n", self.host).as_bytes())
            .await?;
        buf_writer.write_all(b"Connection: keep-alive\r\n").await?;
        for line in self.static_headers {
            debug_assert!(line.ends_with("\r\n"));
            buf_writer.write_all(line.as_bytes()).await?;
        }
        for line in &self.dyn_headers {
            buf_writer.write_all(line.as_bytes()).await?;
        }
        buf_writer.write_all(b"\r\n").await?;
        buf_writer.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Helper function to create a test UpstreamAddr
    fn create_test_upstream_addr() -> UpstreamAddr {
        UpstreamAddr::from_str("example.com:8080").unwrap()
    }

    #[test]
    fn new() {
        let host = create_test_upstream_addr();
        let static_headers = vec![
            "User-Agent: test-agent\r\n".to_string(),
            "Accept: */*\r\n".to_string(),
        ];

        let request = HttpConnectRequest::new(&host, &static_headers);

        assert_eq!(request.host.host_str(), "example.com");
        assert_eq!(request.host.port(), 8080);
        assert_eq!(request.static_headers.len(), 2);
        assert!(request.dyn_headers.is_empty());
    }

    #[test]
    fn append_dyn_header() {
        let host = create_test_upstream_addr();
        let static_headers: Vec<String> = Vec::new();
        let mut request = HttpConnectRequest::new(&host, &static_headers);

        // Appending a single dynamic header
        request.append_dyn_header("X-Custom-Header: value1\r\n".to_string());
        assert_eq!(request.dyn_headers.len(), 1);
        assert_eq!(request.dyn_headers[0], "X-Custom-Header: value1\r\n");

        // Appending multiple dynamic headers
        request.append_dyn_header("X-Another-Header: value2\r\n".to_string());
        assert_eq!(request.dyn_headers.len(), 2);
        assert_eq!(request.dyn_headers[1], "X-Another-Header: value2\r\n");
    }

    #[tokio::test]
    async fn send_no_headers() {
        let host = create_test_upstream_addr();
        let static_headers: Vec<String> = Vec::new();
        let request = HttpConnectRequest::new(&host, &static_headers);

        let mut buffer = Vec::new();
        request.send(&mut buffer).await.unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "CONNECT example.com:8080 HTTP/1.1\r\n\
                       Host: example.com:8080\r\n\
                       Connection: keep-alive\r\n\
                       \r\n";

        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn send_with_static_headers() {
        let host = create_test_upstream_addr();
        let static_headers = vec![
            "User-Agent: test-agent\r\n".to_string(),
            "Accept: */*\r\n".to_string(),
        ];

        let request = HttpConnectRequest::new(&host, &static_headers);

        let mut buffer = Vec::new();
        request.send(&mut buffer).await.unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "CONNECT example.com:8080 HTTP/1.1\r\n\
                       Host: example.com:8080\r\n\
                       Connection: keep-alive\r\n\
                       User-Agent: test-agent\r\n\
                       Accept: */*\r\n\
                       \r\n";

        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn send_with_dynamic_headers() {
        let host = create_test_upstream_addr();
        let static_headers: Vec<String> = Vec::new();
        let mut request = HttpConnectRequest::new(&host, &static_headers);

        request.append_dyn_header("X-Custom-Header: value1\r\n".to_string());
        request.append_dyn_header("X-Another-Header: value2\r\n".to_string());

        let mut buffer = Vec::new();
        request.send(&mut buffer).await.unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "CONNECT example.com:8080 HTTP/1.1\r\n\
                       Host: example.com:8080\r\n\
                       Connection: keep-alive\r\n\
                       X-Custom-Header: value1\r\n\
                       X-Another-Header: value2\r\n\
                       \r\n";

        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn send_with_both_header_types() {
        let host = create_test_upstream_addr();
        let static_headers = vec![
            "User-Agent: test-agent\r\n".to_string(),
            "Accept: */*\r\n".to_string(),
        ];

        let mut request = HttpConnectRequest::new(&host, &static_headers);
        request.append_dyn_header("X-Custom-Header: value1\r\n".to_string());
        request.append_dyn_header("X-Another-Header: value2\r\n".to_string());

        let mut buffer = Vec::new();
        request.send(&mut buffer).await.unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "CONNECT example.com:8080 HTTP/1.1\r\n\
                       Host: example.com:8080\r\n\
                       Connection: keep-alive\r\n\
                       User-Agent: test-agent\r\n\
                       Accept: */*\r\n\
                       X-Custom-Header: value1\r\n\
                       X-Another-Header: value2\r\n\
                       \r\n";

        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn send_with_ipv6_host() {
        let host = UpstreamAddr::from_str("[2001:db8::1]:8080").unwrap();
        let static_headers: Vec<String> = Vec::new();
        let request = HttpConnectRequest::new(&host, &static_headers);

        let mut buffer = Vec::new();
        request.send(&mut buffer).await.unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "CONNECT [2001:db8::1]:8080 HTTP/1.1\r\n\
                       Host: [2001:db8::1]:8080\r\n\
                       Connection: keep-alive\r\n\
                       \r\n";

        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn send_flush_behavior() {
        let host = create_test_upstream_addr();
        let static_headers: Vec<String> = Vec::new();
        let request = HttpConnectRequest::new(&host, &static_headers);

        let mut buffer = Vec::new();
        let result = request.send(&mut buffer).await;

        assert!(result.is_ok());
        // Verify that data was actually written to the buffer
        assert!(!buffer.is_empty());
    }
}
