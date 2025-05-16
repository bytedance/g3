/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::io::AsyncRead;

use super::read_all_now::ReadAllNow;

pub trait LimitedReadExt: AsyncRead {
    fn read_all_now<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadAllNow<'a, Self>
    where
        Self: Unpin,
    {
        ReadAllNow::new(self, buf)
    }
}

impl<R: AsyncRead> LimitedReadExt for R {}
