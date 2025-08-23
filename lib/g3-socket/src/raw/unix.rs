/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

use socket2::Socket;

use super::RawSocket;

impl Drop for RawSocket {
    fn drop(&mut self) {
        if let Some(s) = self.inner.take() {
            let _ = s.into_raw_fd();
        }
    }
}

impl Clone for RawSocket {
    fn clone(&self) -> Self {
        match &self.inner {
            Some(s) => Self::from(s),
            None => RawSocket { inner: None },
        }
    }
}

impl<T: AsRawFd> From<&T> for RawSocket {
    fn from(value: &T) -> Self {
        let socket = unsafe { Socket::from_raw_fd(value.as_raw_fd()) };
        RawSocket {
            inner: Some(socket),
        }
    }
}
