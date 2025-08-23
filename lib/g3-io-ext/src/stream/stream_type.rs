/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
