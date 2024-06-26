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
