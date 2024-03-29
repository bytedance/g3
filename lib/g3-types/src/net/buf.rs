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
