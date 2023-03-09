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

use bytes::BytesMut;
use tokio::io::AsyncBufRead;

use super::fill_wait_data::FillWaitData;
use super::fill_wait_eof::FillWaitEof;
use super::limited_read_buf_until::LimitedReadBufUntil;
use super::limited_read_until::LimitedReadUntil;
use super::limited_skip_until::LimitedSkipUntil;

pub trait LimitedBufReadExt: AsyncBufRead {
    fn limited_read_until<'a>(
        &'a mut self,
        delimiter: u8,
        max_len: usize,
        buf: &'a mut Vec<u8>,
    ) -> LimitedReadUntil<'a, Self>
    where
        Self: Unpin,
    {
        LimitedReadUntil::new(self, delimiter, max_len, buf)
    }

    fn limited_read_buf_until<'a>(
        &'a mut self,
        delimiter: u8,
        max_len: usize,
        buf: &'a mut BytesMut,
    ) -> LimitedReadBufUntil<'a, Self>
    where
        Self: Unpin,
    {
        LimitedReadBufUntil::new(self, delimiter, max_len, buf)
    }

    fn limited_skip_until(&mut self, delimiter: u8, max_len: usize) -> LimitedSkipUntil<Self>
    where
        Self: Unpin,
    {
        LimitedSkipUntil::new(self, delimiter, max_len)
    }

    /// return Poll::Ready(Ok(())) if read ready but no data can be read
    /// return Poll::Ready(Err(e)) if read io error
    fn fill_wait_eof(&mut self) -> FillWaitEof<Self>
    where
        Self: Unpin,
    {
        FillWaitEof::new(self)
    }

    fn fill_wait_data(&mut self) -> FillWaitData<Self>
    where
        Self: Unpin,
    {
        FillWaitData::new(self)
    }
}

impl<R: AsyncBufRead + ?Sized> LimitedBufReadExt for R {}
