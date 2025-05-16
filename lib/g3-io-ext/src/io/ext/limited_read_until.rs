/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::AsyncBufRead;

pub struct LimitedReadUntil<'a, R: ?Sized> {
    reader: &'a mut R,
    delimiter: u8,
    buf: &'a mut Vec<u8>,
    read: usize,
    limit: usize,
}

impl<'a, R> LimitedReadUntil<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    pub(super) fn new(
        reader: &'a mut R,
        delimiter: u8,
        max_len: usize,
        buf: &'a mut Vec<u8>,
    ) -> Self {
        Self {
            reader,
            delimiter,
            buf,
            read: 0,
            limit: max_len,
        }
    }
}

fn read_until_internal<R: AsyncBufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    delimiter: u8,
    buf: &mut Vec<u8>,
    read: &mut usize,
    limit: usize,
) -> Poll<io::Result<(bool, usize)>> {
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
            if let Some(i) = memchr::memchr(delimiter, available) {
                buf.extend_from_slice(&available[..=i]);
                (true, i + 1)
            } else {
                buf.extend_from_slice(available);
                (false, available.len())
            }
        };
        reader.as_mut().consume(used);
        *read += used;
        if done {
            return if *read > limit {
                Poll::Ready(Ok((false, mem::replace(read, 0))))
            } else {
                Poll::Ready(Ok((true, mem::replace(read, 0))))
            };
        }
        if used == 0 || *read > limit {
            return Poll::Ready(Ok((false, mem::replace(read, 0))));
        }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for LimitedReadUntil<'_, R> {
    type Output = io::Result<(bool, usize)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            delimiter,
            buf,
            read,
            limit,
        } = &mut *self;
        read_until_internal(Pin::new(reader), cx, *delimiter, buf, read, *limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn read_single_to_end() {
        let content = b"test body\n";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut out_buf = Vec::<u8>::with_capacity(16);

        let limited_reader = LimitedReadUntil::new(&mut buf_stream, b'\n', 128, &mut out_buf);
        let (found, size) = limited_reader.await.unwrap();
        assert!(found);
        assert_eq!(size, 10);

        let limited_reader = LimitedReadUntil::new(&mut buf_stream, b'\n', 128, &mut out_buf);
        let (found, size) = limited_reader.await.unwrap();
        assert!(!found);
        assert_eq!(size, 0);
    }

    #[tokio::test]
    async fn read_single_too_large() {
        let content = b"test body\n";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut out_buf = Vec::<u8>::with_capacity(16);

        let limited_reader = LimitedReadUntil::new(&mut buf_stream, b'\n', 8, &mut out_buf);
        let (found, size) = limited_reader.await.unwrap();
        assert!(!found);
        assert!(size >= 8);
    }
}
