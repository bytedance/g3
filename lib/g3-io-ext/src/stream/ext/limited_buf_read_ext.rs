/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::BytesMut;
use tokio::io::AsyncBufRead;

use super::fill_wait_data::FillWaitData;
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

    fn limited_skip_until(&mut self, delimiter: u8, max_len: usize) -> LimitedSkipUntil<'_, Self>
    where
        Self: Unpin,
    {
        LimitedSkipUntil::new(self, delimiter, max_len)
    }

    /// Wait for data on Buffered IO Reader
    ///
    /// return Poll::Ready(Ok(true)) if some data can be read
    /// return Poll::Ready(Ok(false)) if read ready but no data can be read
    /// return Poll::Ready(Err(e)) if read io error
    fn fill_wait_data(&mut self) -> FillWaitData<'_, Self>
    where
        Self: Unpin,
    {
        FillWaitData::new(self)
    }
}

impl<R: AsyncBufRead + ?Sized> LimitedBufReadExt for R {}
