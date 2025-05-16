/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectError {
    #[error("connection refused")]
    ConnectionRefused,
    #[error("connection reset")]
    ConnectionReset,
    #[error("network unreachable")]
    NetworkUnreachable,
    #[error("host unreachable")] // from ICMP or local route
    HostUnreachable,
    #[error("timed out")]
    TimedOut,
    #[error("unspecified error: {0:?}")]
    UnspecifiedError(io::Error),
}

impl From<io::Error> for ConnectError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::ConnectionRefused => return ConnectError::ConnectionRefused,
            io::ErrorKind::ConnectionReset => return ConnectError::ConnectionReset,
            io::ErrorKind::HostUnreachable => return ConnectError::HostUnreachable,
            io::ErrorKind::NetworkUnreachable => return ConnectError::NetworkUnreachable,
            io::ErrorKind::TimedOut => return ConnectError::TimedOut,
            _ => {}
        }
        if let Some(code) = e.raw_os_error() {
            match code {
                libc::ENETUNREACH => return ConnectError::NetworkUnreachable,
                libc::ECONNRESET => return ConnectError::ConnectionReset,
                libc::ETIMEDOUT => return ConnectError::TimedOut,
                libc::ECONNREFUSED => return ConnectError::ConnectionRefused,
                libc::EHOSTUNREACH => return ConnectError::HostUnreachable,
                _ => {}
            }
        }
        ConnectError::UnspecifiedError(e)
    }
}
