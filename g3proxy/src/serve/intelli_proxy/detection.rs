/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use tokio::net::TcpStream;

pub(super) enum DetectedProxyProtocol {
    Unknown,
    Http,
    Socks,
}

pub(super) async fn detect_tcp_proxy_protocol(
    stream: &TcpStream,
) -> io::Result<DetectedProxyProtocol> {
    let mut buf = [0u8; 1];
    let len = stream.peek(&mut buf).await?;
    if len == 0 {
        return Ok(DetectedProxyProtocol::Unknown);
    }

    match buf[0] {
        b'\x04' | b'\x05' => return Ok(DetectedProxyProtocol::Socks),
        b'G' | b'H' | b'P' | b'D' | b'C' | b'O' | b'T' => return Ok(DetectedProxyProtocol::Http),
        _ => {}
    }

    Ok(DetectedProxyProtocol::Unknown)
}
