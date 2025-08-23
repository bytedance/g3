/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::error::Error;
use std::{fmt, io};

use openssl::ssl;

pub(crate) trait ConvertSslError {
    fn build_io_error(self, action: SslErrorAction) -> io::Error;
}

#[derive(Debug)]
pub(crate) enum SslErrorAction {
    Accept,
    Connect,
    Read,
    Peek,
    Write,
    Shutdown,
}

impl SslErrorAction {
    fn as_str(&self) -> &'static str {
        match self {
            SslErrorAction::Accept => "accept",
            SslErrorAction::Connect => "connect",
            SslErrorAction::Read => "read",
            SslErrorAction::Peek => "peek",
            SslErrorAction::Write => "write",
            SslErrorAction::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug)]
pub struct SslError {
    action: SslErrorAction,
    inner: ssl::Error,
}

impl ConvertSslError for ssl::Error {
    fn build_io_error(self, action: SslErrorAction) -> io::Error {
        self.into_io_error()
            .unwrap_or_else(|e| io::Error::other(SslError { action, inner: e }))
    }
}

impl fmt::Display for SslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ssl {}: {}", self.action.as_str(), self.inner)
    }
}

impl Error for SslError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.inner.source()
    }
}
