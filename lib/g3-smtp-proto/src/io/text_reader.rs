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

struct EndChecker {
    check_empty: bool,
    check_r_n: bool,
    check_n: bool,
    found: bool,
}

impl Default for EndChecker {
    fn default() -> Self {
        EndChecker {
            check_empty: true,
            check_r_n: false,
            check_n: false,
            found: false,
        }
    }
}

impl EndChecker {
    #[inline]
    fn found(&self) -> bool {
        self.found
    }

    fn check(&mut self, data: &[u8]) -> usize {
        let mut offset = 0;

        loop {
            let left = &data[offset..];
            match left.len() {
                0 => return offset,
                1 => {
                    if self.check_n {
                        self.check_n = false;
                        if left[0] == b'\n' {
                            self.found = true;
                            return offset + 1;
                        }
                    } else if self.check_r_n {
                        self.check_r_n = false;
                        if left[0] == b'\r' {
                            self.check_n = true;
                            return offset + 1;
                        }
                    } else if self.check_empty {
                        self.check_empty = false;
                        if left[0] == b'.' {
                            self.check_r_n = true;
                            return offset + 1;
                        }
                    }
                }
                2 => {
                    if self.check_n {
                        self.check_n = false;
                        if left[0] == b'\n' {
                            self.found = true;
                            return offset + 1;
                        }
                    } else if self.check_r_n {
                        self.check_r_n = false;
                        if left == b"\r\n" {
                            self.found = true;
                            return offset + 2;
                        }
                    } else if self.check_empty {
                        self.check_empty = false;
                        if left == b".\r" {
                            self.check_n = true;
                            return offset + 2;
                        }
                    }
                }
                _ => {
                    if self.check_n {
                        self.check_n = false;
                        if left[0] == b'\n' {
                            self.found = true;
                            return offset + 1;
                        }
                    } else if self.check_r_n {
                        self.check_r_n = false;
                        if left.starts_with(b"\r\n") {
                            self.found = true;
                            return offset + 2;
                        }
                    } else if self.check_empty {
                        self.check_empty = false;
                        if left.starts_with(b".\r\n") {
                            self.found = true;
                            return offset + 3;
                        }
                    }
                }
            }

            if let Some(p) = memchr::memchr(b'\n', left) {
                // skip to next line
                offset += p + 1;
                self.check_empty = true;
            } else {
                return offset + left.len();
            }
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

    pub fn finished(&self) -> bool {
        self.ending.found
    }
}

impl<R> AsyncRead for TextDataReader<'_, R>
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
        let mut read_buf = ReadBuf::new(buf.initialize_unfilled());
        ready!(Pin::new(&mut self.inner).poll_read(cx, &mut read_buf))?;
        let read = read_buf.filled();
        if !read.is_empty() {
            let nr = self.ending.check(read);
            buf.advance(nr);
        }
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use tokio::io::{AsyncReadExt, BufReader, Result};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn read_single_normal() {
        let body_len: usize = 22;
        let content = b"Line 1\r\n\r\n.Line 2\r\n.\r\n";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataReader::new(&mut buf_stream);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], content);
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_single_malformed() {
        let body_len: usize = 22;
        let content = b"Line 1\r\n\r\n.Line 2\r\n.\r\n123";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataReader::new(&mut buf_stream);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_multi_normal() {
        let body_len: usize = 22;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataReader::new(&mut buf_stream);

        let mut buf = Vec::with_capacity(body_len);
        let len = tokio::io::copy(&mut body_deocder, &mut buf).await.unwrap();
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n.Line 2\r\n.\r\n");
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_multi_malformed() {
        let body_len: usize = 22;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n123";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataReader::new(&mut buf_stream);

        let mut buf = Vec::with_capacity(body_len);
        let len = tokio::io::copy(&mut body_deocder, &mut buf).await.unwrap();
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n.Line 2\r\n.\r\n");
        assert!(body_deocder.finished());
    }
}
