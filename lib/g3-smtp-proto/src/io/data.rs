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

use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

const END_CHARS: &[u8] = b"\r\n.\r\n";
const END_SIZE: usize = 16; // big enough for END_CHARS

struct EndChecker {
    buf: [u8; END_SIZE],
    buf_end: usize,
    check_empty: bool,
    found: bool,
}

impl Default for EndChecker {
    fn default() -> Self {
        EndChecker {
            buf: [0u8; END_SIZE],
            buf_end: 0,
            check_empty: true,
            found: false,
        }
    }
}

impl EndChecker {
    #[inline]
    fn found(&self) -> bool {
        self.found
    }

    fn check(&mut self, data: &[u8]) {
        if self.check_empty {
            self.check_empty = false;
            if data == b".\r\n\r\n" {
                self.found = true;
                return;
            }
        }

        if data.len() > END_CHARS.len() {
            if data.ends_with(END_CHARS) {
                self.found = true;
                return;
            }

            let copy_start = data.len() - END_CHARS.len() + 1;
            let copy_src = &data[copy_start..];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    copy_src.as_ptr(),
                    self.buf.as_mut_ptr(),
                    copy_src.len(),
                )
            }
            self.buf_end = copy_src.len();
        } else {
            let copy_dst = &mut self.buf[self.buf_end..];
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), copy_dst.as_mut_ptr(), data.len())
            };
            self.buf_end += data.len();
            if self.buf_end < END_CHARS.len() {
                return;
            }

            if self.buf[..self.buf_end].ends_with(END_CHARS) {
                self.found = true;
                return;
            }

            let copy_len = END_CHARS.len() - 1;
            let copy_start = self.buf.len() - copy_len;
            self.buf.copy_within(copy_start.., 0);
            self.buf_end = copy_len;
        }
    }
}

pub struct TextDataReader<'a, R> {
    inner: &'a mut R,
    ending: EndChecker,
}

impl<'a, R> TextDataReader<'a, R> {
    pub fn new(inner: &'a mut R) -> Self {
        TextDataReader {
            inner,
            ending: EndChecker::default(),
        }
    }
}

impl<'a, R> AsyncRead for TextDataReader<'a, R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.ending.found() {
            return Poll::Ready(Ok(()));
        }
        let start = buf.filled().len();
        ready!(Pin::new(&mut self.inner).poll_read(cx, buf))?;
        let read = &buf.filled()[start..];
        if !read.is_empty() {
            self.ending.check(read);
        }
        Poll::Ready(Ok(()))
    }
}
