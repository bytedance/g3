/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncBufRead, AsyncWrite};

use g3_types::net::{HttpAuth, UpstreamAddr};

use super::{HttpConnectError, HttpConnectRequest, HttpConnectResponse};

pub async fn http_connect_to<S>(
    buf_stream: &mut S,
    auth: &HttpAuth,
    addr: &UpstreamAddr,
) -> Result<(), HttpConnectError>
where
    S: AsyncBufRead + AsyncWrite + Unpin,
{
    let mut req = HttpConnectRequest::new(addr, &[]);

    match auth {
        HttpAuth::None => {}
        HttpAuth::Basic(a) => {
            let line = crate::header::proxy_authorization_basic(&a.username, &a.password);
            req.append_dyn_header(line);
        }
    }

    req.send(buf_stream)
        .await
        .map_err(HttpConnectError::WriteFailed)?;

    let _ = HttpConnectResponse::recv(buf_stream, 2048).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::auth::{Password, Username};
    use g3_types::net::HttpBasicAuth;
    use std::io;
    use std::str::FromStr;
    use tokio::io::BufReader;
    use tokio_test::io::Builder;

    #[tokio::test]
    async fn http_connect_to_success_no_auth() {
        let stream = Builder::new()
            .write(b"CONNECT example.com:8080 HTTP/1.1\r\n")
            .write(b"Host: example.com:8080\r\n")
            .write(b"Connection: keep-alive\r\n")
            .write(b"\r\n")
            .read(b"HTTP/1.1 200 OK\r\n\r\n")
            .build();

        let mut stream = BufReader::new(stream);
        let addr = UpstreamAddr::from_str("example.com:8080").unwrap();
        let auth = HttpAuth::None;

        let result = http_connect_to(&mut stream, &auth, &addr).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn http_connect_to_success_basic_auth() {
        let stream = Builder::new()
            .write(b"CONNECT example.com:8080 HTTP/1.1\r\n")
            .write(b"Host: example.com:8080\r\n")
            .write(b"Connection: keep-alive\r\n")
            .write(b"Proxy-Authorization: Basic dXNlcjpwYXNz\r\n")
            .write(b"\r\n")
            .read(b"HTTP/1.1 200 OK\r\n\r\n")
            .build();

        let mut stream = BufReader::new(stream);
        let addr = UpstreamAddr::from_str("example.com:8080").unwrap();

        let username = Username::from_encoded("user").unwrap();
        let password = Password::from_encoded("pass").unwrap();
        let basic_auth = HttpBasicAuth::new(username, password);
        let auth = HttpAuth::Basic(basic_auth);

        let result = http_connect_to(&mut stream, &auth, &addr).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn http_connect_to_write_failed() {
        let stream = Builder::new()
            .write(b"CONNECT example.com:8080 HTTP/1.1\r\n")
            .write_error(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"))
            .build();

        let mut stream = BufReader::new(stream);
        let addr = UpstreamAddr::from_str("example.com:8080").unwrap();
        let auth = HttpAuth::None;

        let err = http_connect_to(&mut stream, &auth, &addr)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::WriteFailed(_)));
    }

    #[tokio::test]
    async fn http_connect_to_read_failed() {
        let stream = Builder::new()
            .write(b"CONNECT example.com:8080 HTTP/1.1\r\n")
            .write(b"Host: example.com:8080\r\n")
            .write(b"Connection: keep-alive\r\n")
            .write(b"\r\n")
            .read_error(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "read failed",
            ))
            .build();

        let mut stream = BufReader::new(stream);
        let addr = UpstreamAddr::from_str("example.com:8080").unwrap();
        let auth = HttpAuth::None;

        let err = http_connect_to(&mut stream, &auth, &addr)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectError::ReadFailed(_)));
    }

    #[tokio::test]
    async fn http_connect_to_unexpected_status_code() {
        let stream = Builder::new()
            .write(b"CONNECT example.com:8080 HTTP/1.1\r\n")
            .write(b"Host: example.com:8080\r\n")
            .write(b"Connection: keep-alive\r\n")
            .write(b"\r\n")
            .read(b"HTTP/1.1 404 Not Found\r\n\r\n")
            .build();

        let mut stream = BufReader::new(stream);
        let addr = UpstreamAddr::from_str("example.com:8080").unwrap();
        let auth = HttpAuth::None;

        let err = http_connect_to(&mut stream, &auth, &addr)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            HttpConnectError::UnexpectedStatusCode(404, _)
        ));
    }
}
