/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
    impl_static!(AUTHENTICATION_REQUIRED, "530 Authentication required\r\n");

    pub fn local_service_closing(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("221 [{v4}] Service closing transmission channel\r\n"),
            IpAddr::V6(v6) => format!("221 Ipv6:{v6} Service closing transmission channel\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn internal_server_error(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => {
                format!("421 [{v4}] Internal server error, closing transmission channel\r\n")
            }
            IpAddr::V6(v6) => {
                format!("421 Ipv6:{v6} Internal server error, closing transmission channel\r\n")
            }
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn local_service_not_available(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => {
                format!("421 [{v4}] Service not available, closing transmission channel\r\n")
            }
            IpAddr::V6(v6) => {
                format!("421 Ipv6:{v6} Service not available, closing transmission channel\r\n")
            }
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn message_blocked(local_ip: IpAddr, reason: String) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => {
                format!("421 [{v4}] Service not available, message blocked: {reason}\r\n")
            }
            IpAddr::V6(v6) => {
                format!("421 Ipv6:{v6} Service not available, message blocked: {reason}\r\n")
            }
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn local_service_blocked(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Service not ready - protocol blocked\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Service not ready - protocol blocked\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_service_not_ready(local_ip: IpAddr, reason: &str) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream service not ready - {reason}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream service not ready - {reason}\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_io_error(local_ip: IpAddr, e: &io::Error) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream io error: {e}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream io error: {e}\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_io_closed(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream io closed\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream io closed\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_line_too_long(local_ip: IpAddr) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream io closed\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream io closed\r\n"),
        };
        ResponseEncoder::Owned(msg)
    }

    pub fn upstream_response_error(local_ip: IpAddr, e: &ResponseLineError) -> Self {
        let msg = match local_ip {
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream response error: {e}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream response error: {e}\r\n"),
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
