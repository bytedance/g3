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

use std::future::Future;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::AsyncWrite;

pub struct WriteAllVectored<'a, 'b, W: ?Sized, const N: usize> {
    writer: &'a mut W,
    bufs: [IoSlice<'b>; N],
    bufs_skip: usize,
    current_offset: usize,
    total_offset: u64,
}

impl<'a, 'b, W: ?Sized, const N: usize> WriteAllVectored<'a, 'b, W, N> {
    pub(crate) fn new(writer: &'a mut W, bufs: [IoSlice<'b>; N]) -> Self {
        WriteAllVectored {
            writer,
            bufs,
            bufs_skip: 0,
            current_offset: 0,
            total_offset: 0,
        }
    }
}

fn write_all_vectored_internal<W: AsyncWrite + ?Sized, const N: usize>(
    mut writer: Pin<&mut W>,
    cx: &mut Context<'_>,
    bufs: &[IoSlice<'_>; N],
    bufs_skip: &mut usize,
    current_offset: &mut usize,
    total_offset: &mut u64,
) -> Poll<io::Result<u64>> {
    use std::cmp::Ordering;

    loop {
        let mut w_bufs = [IoSlice::new(b""); N];
        let mut w_index = 0;
        for (i, b) in bufs.iter().enumerate() {
            match i.cmp(bufs_skip) {
                Ordering::Less => {}
                Ordering::Equal => {
                    w_bufs[w_index] = IoSlice::new(&b.as_ref()[*current_offset..]);
                    w_index += 1;
                }
                Ordering::Greater => {
                    w_bufs[w_index] = IoSlice::new(b.as_ref());
                    w_index += 1;
                }
            }
        }
        if w_index == 0 {
            break;
        }
        let w_bufs = &w_bufs[0..w_index];

        let mut nw = ready!(writer.as_mut().poll_write_vectored(cx, w_bufs))?;
        *total_offset += nw as u64;
        for v in w_bufs {
            match v.len().cmp(&nw) {
                Ordering::Less => {
                    nw -= v.len();
                    *bufs_skip += 1;
                    *current_offset = 0;
                }
                Ordering::Equal => {
                    *bufs_skip += 1;
                    *current_offset = 0;
                    break;
                }
                Ordering::Greater => {
                    *current_offset += nw;
                    break;
                }
            }
        }
    }

    Poll::Ready(Ok(*total_offset))
}

impl<W, const N: usize> Future for WriteAllVectored<'_, '_, W, N>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        let Self {
            writer,
            bufs,
            bufs_skip,
            current_offset,
            total_offset,
        } = &mut *self;
        write_all_vectored_internal(
            Pin::new(writer),
            cx,
            bufs,
            bufs_skip,
            current_offset,
            total_offset,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    struct Writer {
        buf: Vec<u8>,
        batch: usize,
    }

    impl Writer {
        fn new(batch: usize) -> Self {
            Writer {
                buf: Vec::new(),
                batch,
            }
        }
    }

    impl AsyncWrite for Writer {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, Error>> {
            if buf.len() < self.batch {
                self.buf.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            } else {
                let len = self.batch;
                self.buf.extend_from_slice(&buf[0..len]);
                Poll::Ready(Ok(len))
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn t2_eq_le() {
        let mut writer = Writer::new(4);
        let fut = WriteAllVectored::new(&mut writer, [IoSlice::new(b"1234"), IoSlice::new(b"123")]);
        let nw = fut.await.unwrap();
        assert_eq!(nw, 7);
        assert_eq!(writer.buf.as_slice(), b"1234123");
    }

    #[tokio::test]
    async fn t4_eq_le_ge_le() {
        let mut writer = Writer::new(4);
        let fut = WriteAllVectored::new(
            &mut writer,
            [
                IoSlice::new(b"1234"),
                IoSlice::new(b"123"),
                IoSlice::new(b"12345"),
                IoSlice::new(b"1"),
            ],
        );
        let nw = fut.await.unwrap();
        assert_eq!(nw, 13);
        assert_eq!(writer.buf.as_slice(), b"1234123123451");
    }

    #[tokio::test]
    async fn t4_ge_ge_le_eq() {
        let mut writer = Writer::new(4);
        let fut = WriteAllVectored::new(
            &mut writer,
            [
                IoSlice::new(b"12345678"),
                IoSlice::new(b"12345"),
                IoSlice::new(b"1"),
                IoSlice::new(b"1234"),
            ],
        );
        let nw = fut.await.unwrap();
        assert_eq!(nw, 18);
        assert_eq!(writer.buf.as_slice(), b"123456781234511234");
    }

    #[tokio::test]
    async fn t4_le_le_le_eq() {
        let mut writer = Writer::new(4);
        let fut = WriteAllVectored::new(
            &mut writer,
            [
                IoSlice::new(b"1"),
                IoSlice::new(b"12"),
                IoSlice::new(b"123"),
                IoSlice::new(b"1234"),
            ],
        );
        let nw = fut.await.unwrap();
        assert_eq!(nw, 10);
        assert_eq!(writer.buf.as_slice(), b"1121231234");
    }
}
