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

use tokio::io::{AsyncRead, AsyncWrite, Join};
use tokio::net::{TcpStream, tcp};

pub trait AsyncStream {
    type R;
    type W;

    fn into_split(self) -> (Self::R, Self::W);
}

impl AsyncStream for TcpStream {
    type R = tcp::OwnedReadHalf;
    type W = tcp::OwnedWriteHalf;

    fn into_split(self) -> (Self::R, Self::W) {
        self.into_split()
    }
}

impl<R, W> AsyncStream for Join<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    type R = R;
    type W = W;

    fn into_split(self) -> (Self::R, Self::W) {
        self.into_inner()
    }
}

#[cfg(feature = "openssl")]
pub mod openssl;

#[cfg(feature = "rustls")]
pub mod rustls;
