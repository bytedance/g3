/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket};

use socket2::Socket;

use super::RawSocket;

impl Drop for RawSocket {
    fn drop(&mut self) {
        if let Some(s) = self.inner.take() {
            let _ = s.into_raw_socket();
        }
    }
}

impl Clone for RawSocket {
    fn clone(&self) -> Self {
        if let Some(s) = &self.inner {
            Self::from(s)
        } else {
            RawSocket { inner: None }
        }
    }
}

impl<T: AsRawSocket> From<&T> for RawSocket {
    fn from(value: &T) -> Self {
        let socket = unsafe { Socket::from_raw_socket(value.as_raw_socket()) };
        RawSocket {
            inner: Some(socket),
        }
    }
}
