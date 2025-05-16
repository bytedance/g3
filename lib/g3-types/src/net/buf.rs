/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SocketBufferConfig {
    recv: Option<usize>,
    send: Option<usize>,
}

impl SocketBufferConfig {
    pub fn new(size: usize) -> Self {
        SocketBufferConfig {
            recv: Some(size),
            send: Some(size),
        }
    }

    #[inline]
    pub fn set_recv_size(&mut self, size: usize) {
        self.recv = Some(size);
    }

    #[inline]
    pub fn recv_size(&self) -> Option<usize> {
        self.recv
    }

    #[inline]
    pub fn set_send_size(&mut self, size: usize) {
        self.send = Some(size);
    }

    #[inline]
    pub fn send_size(&self) -> Option<usize> {
        self.send
    }
}
