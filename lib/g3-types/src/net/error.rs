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
