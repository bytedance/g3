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
