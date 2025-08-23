/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io;
use tokio::io::{AsyncRead, ReadBuf};

pub struct ReadAllNow<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<'a, R> ReadAllNow<'a, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        ReadAllNow { reader, buf }
    }
}

fn read_all_now_internal<R: AsyncRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
) -> Poll<io::Result<Option<usize>>> {
    let mut buf = ReadBuf::new(buf);
    loop {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(Some(buf.filled().len())));
        }
        let old_filled_len = buf.filled().len();
        match reader.as_mut().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(_)) => {
                let filled_len = buf.filled().len();
                if filled_len == 0 {
                    return Poll::Ready(Ok(None));
                }
                if filled_len == old_filled_len {
                    return Poll::Ready(Ok(Some(filled_len)));
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Ready(Ok(Some(buf.filled().len()))),
        }
    }
}

impl<R> Future for ReadAllNow<'_, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<Option<usize>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ReadAllNow { reader, buf } = &mut *self;
        read_all_now_internal(Pin::new(reader), cx, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed() {
        let mut stream = tokio_test::io::Builder::new().read(&[]).build();
        let mut buf = vec![0u8; 1024];
        assert!(
            ReadAllNow::new(&mut stream, &mut buf)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn read_one() {
        let buf1 = b"123456";
        let mut stream = tokio_test::io::Builder::new().read(buf1).build();
        let mut buf = vec![0u8; 1024];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, buf1.len());
        assert_eq!(&buf[..nr], buf1);
    }

    #[tokio::test]
    async fn read_two() {
        let buf1 = b"123456";
        let buf2 = b"abcdef";
        let mut stream = tokio_test::io::Builder::new().read(buf1).read(buf2).build();
        let mut buf = vec![0u8; 1024];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, buf1.len() + buf2.len());
        assert_eq!(&buf[..buf1.len()], buf1);
        assert_eq!(&buf[buf1.len()..nr], buf2);
    }

    #[tokio::test]
    async fn read_one_frag() {
        let buf1 = b"123456";
        let mut stream = tokio_test::io::Builder::new().read(buf1).build();
        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, 4);
        assert_eq!(&buf[..nr], &buf1[..nr]);

        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, 2);
        assert_eq!(&buf[..nr], &buf1[4..]);
    }

    #[tokio::test]
    async fn read_two_frag() {
        let buf1 = b"123456";
        let buf2 = b"abcdef";
        let mut stream = tokio_test::io::Builder::new().read(buf1).read(buf2).build();
        let mut buf = vec![0u8; 10];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, 10);
        assert_eq!(&buf[..nr], b"123456abcd");

        let mut buf = vec![0u8; 4];
        let nr = ReadAllNow::new(&mut stream, &mut buf)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(nr, 2);
        assert_eq!(&buf[..nr], b"ef");
    }
}
