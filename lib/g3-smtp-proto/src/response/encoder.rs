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
            IpAddr::V4(v4) => format!("554 [{v4}] Upstream line too long\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} Upstream line too long\r\n"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    // Helper function to create IPv4 address
    fn ipv4_addr(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    // Helper function to create IPv6 address
    fn ipv6_addr(segments: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        ))
    }

    // Helper function to get response bytes as string for easier testing
    fn response_as_string(encoder: &ResponseEncoder) -> String {
        String::from_utf8(encoder.as_bytes().to_vec()).unwrap()
    }

    #[test]
    fn all_methods() {
        let encoder = ResponseEncoder::local_service_closing(ipv4_addr(192, 168, 1, 1));
        assert_eq!(
            response_as_string(&encoder),
            "221 [192.168.1.1] Service closing transmission channel\r\n"
        );

        let encoder =
            ResponseEncoder::local_service_closing(ipv6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]));
        assert_eq!(
            response_as_string(&encoder),
            "221 Ipv6:2001:db8::1 Service closing transmission channel\r\n"
        );

        let encoder = ResponseEncoder::internal_server_error(ipv4_addr(0, 0, 0, 0));
        assert_eq!(
            response_as_string(&encoder),
            "421 [0.0.0.0] Internal server error, closing transmission channel\r\n"
        );

        let encoder = ResponseEncoder::internal_server_error(ipv6_addr([0, 0, 0, 0, 0, 0, 0, 1]));
        assert_eq!(
            response_as_string(&encoder),
            "421 Ipv6:::1 Internal server error, closing transmission channel\r\n"
        );

        let encoder = ResponseEncoder::local_service_not_available(ipv4_addr(172, 16, 0, 1));
        assert_eq!(
            response_as_string(&encoder),
            "421 [172.16.0.1] Service not available, closing transmission channel\r\n"
        );

        let encoder = ResponseEncoder::local_service_not_available(ipv6_addr([
            0xfe80, 0, 0, 0, 0x2e0, 0x4cff, 0xfe68, 0x12a0,
        ]));
        assert_eq!(
            response_as_string(&encoder),
            "421 Ipv6:fe80::2e0:4cff:fe68:12a0 Service not available, closing transmission channel\r\n"
        );

        let encoder = ResponseEncoder::message_blocked(
            ipv4_addr(10, 0, 0, 1),
            "rate limit exceeded".to_string(),
        );
        assert_eq!(
            response_as_string(&encoder),
            "421 [10.0.0.1] Service not available, message blocked: rate limit exceeded\r\n"
        );

        let encoder = ResponseEncoder::message_blocked(
            ipv6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]),
            "content filter".to_string(),
        );
        assert_eq!(
            response_as_string(&encoder),
            "421 Ipv6:2001:db8::1 Service not available, message blocked: content filter\r\n"
        );

        let encoder = ResponseEncoder::local_service_blocked(ipv4_addr(203, 0, 113, 1));
        assert_eq!(
            response_as_string(&encoder),
            "554 [203.0.113.1] Service not ready - protocol blocked\r\n"
        );

        let encoder = ResponseEncoder::local_service_blocked(ipv6_addr([
            0x2001, 0xdb8, 0x85a3, 0x8d3, 0x1319, 0x8a2e, 0x370, 0x7348,
        ]));
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:2001:db8:85a3:8d3:1319:8a2e:370:7348 Service not ready - protocol blocked\r\n"
        );

        let encoder = ResponseEncoder::upstream_service_not_ready(
            ipv4_addr(172, 16, 0, 1),
            "server overloaded",
        );
        assert_eq!(
            response_as_string(&encoder),
            "554 [172.16.0.1] Upstream service not ready - server overloaded\r\n"
        );

        let encoder = ResponseEncoder::upstream_service_not_ready(
            ipv6_addr([0, 0, 0, 0, 0, 0, 0, 1]),
            "network unreachable",
        );
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:::1 Upstream service not ready - network unreachable\r\n"
        );

        let io_error = io::Error::new(io::ErrorKind::ConnectionRefused, "Connection refused");
        let encoder = ResponseEncoder::upstream_io_error(ipv4_addr(192, 168, 1, 1), &io_error);
        assert_eq!(
            response_as_string(&encoder),
            "554 [192.168.1.1] Upstream io error: Connection refused\r\n"
        );

        let io_error = io::Error::new(io::ErrorKind::TimedOut, "Operation timed out");
        let encoder =
            ResponseEncoder::upstream_io_error(ipv6_addr([0, 0, 0, 0, 0, 0, 0, 1]), &io_error);
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:::1 Upstream io error: Operation timed out\r\n"
        );

        let encoder = ResponseEncoder::upstream_io_closed(ipv4_addr(198, 51, 100, 1));
        assert_eq!(
            response_as_string(&encoder),
            "554 [198.51.100.1] Upstream io closed\r\n"
        );

        let encoder =
            ResponseEncoder::upstream_io_closed(ipv6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 42]));
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:2001:db8::2a Upstream io closed\r\n"
        );

        let encoder = ResponseEncoder::upstream_line_too_long(ipv4_addr(192, 168, 1, 1));
        assert_eq!(
            response_as_string(&encoder),
            "554 [192.168.1.1] Upstream line too long\r\n"
        );

        let encoder = ResponseEncoder::upstream_line_too_long(ipv6_addr([0, 0, 0, 0, 0, 0, 0, 1]));
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:::1 Upstream line too long\r\n"
        );

        let response_error = ResponseLineError::InvalidCode;
        let encoder =
            ResponseEncoder::upstream_response_error(ipv4_addr(192, 168, 1, 1), &response_error);
        assert_eq!(
            response_as_string(&encoder),
            "554 [192.168.1.1] Upstream response error: invalid code\r\n"
        );

        let response_error = ResponseLineError::TooShort;
        let encoder = ResponseEncoder::upstream_response_error(
            ipv6_addr([0, 0, 0, 0, 0, 0, 0, 1]),
            &response_error,
        );
        assert_eq!(
            response_as_string(&encoder),
            "554 Ipv6:::1 Upstream response error: too short\r\n"
        );

        let encoder = ResponseEncoder::SYNTAX_ERROR;
        let bytes = encoder.as_bytes();
        assert_eq!(bytes, b"500 Syntax error\r\n");
    }
}
