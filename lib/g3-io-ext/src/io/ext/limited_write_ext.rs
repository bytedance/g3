/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::IoSlice;

use tokio::io::AsyncWrite;

use super::write_all_flush::WriteAllFlush;
use super::write_all_vectored::WriteAllVectored;

pub trait LimitedWriteExt: AsyncWrite {
    fn write_all_vectored<'a, 'b, const N: usize>(
        &'a mut self,
        bufs: [IoSlice<'b>; N],
    ) -> WriteAllVectored<'a, 'b, Self, N>
    where
        Self: Unpin,
    {
        WriteAllVectored::new(self, bufs)
    }

    fn write_all_flush<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFlush<'a, Self>
    where
        Self: Unpin,
    {
        WriteAllFlush::new(self, buf)
    }
}

impl<W: AsyncWrite + ?Sized> LimitedWriteExt for W {}
