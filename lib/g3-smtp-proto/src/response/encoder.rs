/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::net::IpAddr;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::response::ResponseLineError;

pub enum ResponseEncoder {
    Static(&'static str),
    Owned(String),
}

macro_rules! impl_static {
    ($name:ident, $msg:literal) => {
        pub const $name: ResponseEncoder = ResponseEncoder::Static($msg);
    };
}

impl ResponseEncoder {
    impl_static!(SYNTAX_ERROR, "500 Syntax error\r\n");
    impl_static!(COMMAND_UNRECOGNIZED, "500 Command unrecognized\r\n");
    impl_static!(COMMAND_LINE_TOO_LONG, "500 Line too long\r\n");
    impl_static!(COMMAND_NOT_IMPLEMENTED, "502 Command not implemented\r\n");
    impl_static!(BAD_SEQUENCE_OF_COMMANDS, "503 Bad sequence of commands\r\n");
    impl_static!(
        COMMAND_PARAMATER_NOT_IMPLEMENTED,
        "504 Command parameter not implemented\r\n"
    );

    pub fn local_service_closing(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("221 [{v4}] Service closing transmission channel\r\n"),
            IpAddr::V6(v6) => format!("221 Ipv6:{v6} Service closing transmission channel\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn local_service_blocked(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] service not ready - protocol blocked\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} service not ready - protocol blocked\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_service_not_ready(local_ip: IpAddr, reason: &str) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream service not ready - {reason}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream service not ready - {reason}\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_io_error(local_ip: IpAddr, e: &io::Error) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream io error: {e}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream io error: {e}\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_io_closed(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream io closed\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream io closed\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_line_too_long(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream io closed\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream io closed\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_response_error(local_ip: IpAddr, e: &ResponseLineError) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream response error: {e}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream response error: {e}\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ResponseEncoder::Static(s) => s.as_bytes(),
            ResponseEncoder::Owned(s) => s.as_bytes(),
        }
    }

    pub async fn write<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all(self.as_bytes()).await?;
        writer.flush().await
    }
}
