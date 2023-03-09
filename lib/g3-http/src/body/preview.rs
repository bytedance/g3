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
use std::io::{self, Write};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use atoi::FromRadix16;
use bytes::BufMut;
use thiserror::Error;
use tokio::io::AsyncBufRead;

use super::HttpBodyType;

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("read error: {0:?}")]
    ReadError(#[from] io::Error),
    #[error("reader closed")]
    ReaderClosed,
    #[error("preview data already polled")]
    AlreadyPolled,
    #[error("invalid chunked body")]
    InvalidChunkedBody,
}

pub struct PreviewData<'a, R> {
    pub header: Option<Vec<u8>>,
    pub body_type: HttpBodyType,
    pub limit: usize,
    pub inner: &'a mut R,
}

impl<'a, R> Future for PreviewData<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = Result<(Vec<u8>, PreviewDataState), PreviewError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(mut header) = self.header.take() {
            let limit = self.limit;
            let body_type = self.body_type;
            let buf = ready!(Pin::new(&mut *self.inner).poll_fill_buf(cx))
                .map_err(PreviewError::ReadError)?;
            if buf.is_empty() {
                return Poll::Ready(Err(PreviewError::ReaderClosed));
            }
            let state = push_preview_data(&mut header, body_type, limit, buf)?;
            Poll::Ready(Ok((header, state)))
        } else {
            Poll::Ready(Err(PreviewError::AlreadyPolled))
        }
    }
}

pub struct PreviewDataState {
    pub consume_size: usize,
    pub preview_size: usize,
    pub preview_eof: bool,
    pub chunked_next_size: u64,
}

fn push_preview_data(
    header: &mut Vec<u8>,
    body_type: HttpBodyType,
    limit: usize,
    buf: &[u8],
) -> Result<PreviewDataState, PreviewError> {
    match body_type {
        HttpBodyType::ReadUntilEnd => {
            let len = buf.len();
            let preview_size = if len > limit {
                let _ = write!(header, "{limit:x}\r\n");
                header.put_slice(&buf[..limit]);
                limit
            } else {
                let _ = write!(header, "{len:x}\r\n");
                header.put_slice(buf);
                len
            };
            header.put_slice(b"\r\n0\r\n\r\n");
            Ok(PreviewDataState {
                consume_size: preview_size,
                preview_size,
                preview_eof: false,
                chunked_next_size: 0,
            })
        }
        HttpBodyType::ContentLength(total_len) => {
            let len = buf
                .len()
                .min(usize::try_from(total_len).unwrap_or(usize::MAX));
            let mut preview_eof = false;
            let preview_size = if len > limit {
                let _ = write!(header, "{limit:x}\r\n");
                header.put_slice(&buf[..limit]);
                header.put_slice(b"\r\n0\r\n\r\n");
                limit
            } else {
                let _ = write!(header, "{len:x}\r\n");
                header.put_slice(&buf[..len]);
                if (len as u64) < total_len {
                    header.put_slice(b"\r\n0\r\n\r\n");
                } else {
                    header.put_slice(b"\r\n0; ieof\r\n\r\n");
                    preview_eof = true;
                }
                len
            };
            Ok(PreviewDataState {
                consume_size: preview_size,
                preview_size,
                preview_eof,
                chunked_next_size: 0,
            })
        }
        HttpBodyType::ChunkedWithoutTrailer => push_chunked_preview_data(header, limit, buf, false),
        HttpBodyType::ChunkedWithTrailer => push_chunked_preview_data(header, limit, buf, true),
    }
}

fn push_chunked_preview_data(
    header: &mut Vec<u8>,
    limit: usize,
    buf: &[u8],
    has_trailer: bool,
) -> Result<PreviewDataState, PreviewError> {
    let mut consume_size = 0;
    let mut preview_size = 0;

    loop {
        let left = &buf[consume_size..];
        if left.is_empty() {
            break;
        }

        let Some(p) = memchr::memchr(b'\n', left) else {
            break;
        };
        let (chunk_size, offset) = u64::from_radix_16(&left[0..p]);
        if offset == 0 {
            return Err(PreviewError::InvalidChunkedBody);
        } else if offset + 1 == p {
            if left[offset] != b'\r' {
                return Err(PreviewError::InvalidChunkedBody);
            }
        } else if left[offset] != b';' {
            return Err(PreviewError::InvalidChunkedBody);
        }

        if chunk_size == 0 {
            if has_trailer {
                // do not consume the ending chunk, so we can pass Trailer along with the ending chunk in the continue request
                header.put_slice(b"0\r\n\r\n");
                return Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: 0,
                });
            }

            let end = &left[p + 1..];
            let end_len = end.len();
            if end_len >= 1 {
                if end[0] == b'\n' {
                    // end chunk ends
                    consume_size += p + 2;
                    header.put_slice(b"0; ieof\r\n\r\n");
                    return Ok(PreviewDataState {
                        consume_size,
                        preview_size,
                        preview_eof: true,
                        chunked_next_size: 0,
                    });
                }
                if end_len >= 2 && end[0] == b'\r' && end[1] == b'\n' {
                    // end chunk ends
                    consume_size += p + 3;
                    header.put_slice(b"0; ieof\r\n\r\n");
                    return Ok(PreviewDataState {
                        consume_size,
                        preview_size,
                        preview_eof: true,
                        chunked_next_size: 0,
                    });
                }
                return Err(PreviewError::InvalidChunkedBody);
            } else {
                // do not consume the ending chunk, send them in the continue request
                header.put_slice(b"0\r\n\r\n");
                return Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: 0,
                });
            }
        }

        let left_limit = limit - preview_size;
        if left_limit == 0 {
            break;
        }

        let chunk_size_usize = usize::try_from(chunk_size).unwrap_or(usize::MAX);
        if chunk_size_usize <= left_limit {
            let left = &left[p + 1..];
            let left_len = left.len();
            if left_len == 0 {
                // no real data available, skip this chunk
                header.put_slice(b"0\r\n\r\n");
                return Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: 0,
                });
            } else if left_len > chunk_size_usize {
                let _ = write!(header, "{chunk_size_usize:x}\r\n");
                header.put_slice(&left[..chunk_size_usize]);
                header.put_slice(b"\r\n");

                preview_size += chunk_size_usize;
                consume_size += p + 1 + chunk_size_usize;

                let end = &left[chunk_size_usize..];
                let end_len = end.len();

                // end_len must be > 1 as left_len > chunk_size
                if end[0] == b'\n' {
                    consume_size += 1;
                    continue;
                }
                if end_len >= 2 && end[0] == b'\r' && end[1] == b'\n' {
                    consume_size += 2;
                    continue;
                }
                return Err(PreviewError::InvalidChunkedBody);
            } else if left_len == chunk_size_usize {
                // leave one byte to ease the send of the continue request
                let to_preview = left_len - 1;
                return if to_preview > 0 {
                    let _ = write!(header, "{to_preview:x}\r\n");
                    header.put_slice(&left[..to_preview]);
                    header.put_slice(b"\r\n0\r\n\r\n");

                    preview_size += to_preview;
                    consume_size += p + 1 + to_preview;

                    Ok(PreviewDataState {
                        consume_size,
                        preview_size,
                        preview_eof: false,
                        chunked_next_size: chunk_size - to_preview as u64,
                    })
                } else {
                    Ok(PreviewDataState {
                        consume_size,
                        preview_size,
                        preview_eof: false,
                        chunked_next_size: 0,
                    })
                };
            } else {
                let _ = write!(header, "{left_len:x}\r\n");
                header.put_slice(left);
                header.put_slice(b"\r\n0\r\n\r\n");

                preview_size += left_len;
                consume_size += p + 1 + left_len;

                return Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: chunk_size - left_len as u64,
                });
            }
        } else {
            // this chunk is big enough
            let left = &left[p + 1..];
            let left_len = left.len();
            return if left_len > left_limit {
                let _ = write!(header, "{left_limit:x}\r\n");
                header.put_slice(&left[..left_limit]);
                header.put_slice(b"\r\n0\r\n\r\n");

                consume_size += p + 1 + left_limit;
                preview_size += left_limit;

                Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: chunk_size - left_limit as u64,
                })
            } else {
                let _ = write!(header, "{left_len:x}\r\n");
                header.put_slice(left);
                header.put_slice(b"\r\n0\r\n\r\n");

                consume_size += p + 1 + left_len;
                preview_size += left_len;

                Ok(PreviewDataState {
                    consume_size,
                    preview_size,
                    preview_eof: false,
                    chunked_next_size: chunk_size - left_len as u64,
                })
            };
        }
    }

    header.put_slice(b"0\r\n\r\n");
    Ok(PreviewDataState {
        consume_size,
        preview_size,
        preview_eof: false,
        chunked_next_size: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_data_until_end() {
        let mut headers = Vec::with_capacity(256);

        let s = push_preview_data(&mut headers, HttpBodyType::ReadUntilEnd, 4, b"12").unwrap();
        assert_eq!(s.consume_size, 2);
        assert_eq!(s.preview_size, 2);
        assert_eq!(headers.as_slice(), b"2\r\n12\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(&mut headers, HttpBodyType::ReadUntilEnd, 4, b"123456").unwrap();
        assert_eq!(s.consume_size, 4);
        assert_eq!(s.preview_size, 4);
        assert_eq!(headers.as_slice(), b"4\r\n1234\r\n0\r\n\r\n");
    }

    #[test]
    fn preview_data_content_length() {
        let mut headers = Vec::with_capacity(256);

        let s = push_preview_data(&mut headers, HttpBodyType::ContentLength(2), 4, b"12").unwrap();
        assert_eq!(s.consume_size, 2);
        assert_eq!(s.preview_size, 2);
        assert!(s.preview_eof);
        assert_eq!(headers.as_slice(), b"2\r\n12\r\n0; ieof\r\n\r\n");

        headers.clear();
        let s = push_preview_data(&mut headers, HttpBodyType::ContentLength(4), 4, b"12").unwrap();
        assert_eq!(s.consume_size, 2);
        assert_eq!(s.preview_size, 2);
        assert!(!s.preview_eof);
        assert_eq!(headers.as_slice(), b"2\r\n12\r\n0\r\n\r\n");

        headers.clear();
        let s =
            push_preview_data(&mut headers, HttpBodyType::ContentLength(4), 4, b"123456").unwrap();
        assert_eq!(s.consume_size, 4);
        assert_eq!(s.preview_size, 4);
        assert!(s.preview_eof);
        assert_eq!(headers.as_slice(), b"4\r\n1234\r\n0; ieof\r\n\r\n");

        headers.clear();
        let s =
            push_preview_data(&mut headers, HttpBodyType::ContentLength(6), 4, b"123456").unwrap();
        assert_eq!(s.consume_size, 4);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(headers.as_slice(), b"4\r\n1234\r\n0\r\n\r\n");
    }

    #[test]
    fn preview_data_chunked() {
        let mut headers = Vec::with_capacity(256);

        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 0);
        assert_eq!(s.preview_size, 0);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na",
        )
        .unwrap();
        assert_eq!(s.consume_size, 0);
        assert_eq!(s.preview_size, 0);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.len(), 0);

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 6);
        assert_eq!(s.preview_size, 1);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"1\r\na\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na\r\n1\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 6);
        assert_eq!(s.preview_size, 1);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"1\r\na\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na\r\n1\r\nb\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 12);
        assert_eq!(s.preview_size, 2);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"1\r\na\r\n1\r\nb\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na\r\n3\r\nbcd\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 14);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"1\r\na\r\n3\r\nbcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"2\r\nab\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 7);
        assert_eq!(s.preview_size, 2);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"2\r\nab\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"4\r\nabcd\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 9);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"5\r\nabcde\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 7);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 1);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"1\r\na\r\n4\r\nbcde\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 12);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 1);
        assert_eq!(headers.as_slice(), b"1\r\na\r\n3\r\nbcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"3\r\nabc\r\n0",
        )
        .unwrap();
        assert_eq!(s.consume_size, 8);
        assert_eq!(s.preview_size, 3);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"3\r\nabc\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"4\r\nabcd\r\n0",
        )
        .unwrap();
        assert_eq!(s.consume_size, 9);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"4\r\nabcd\r\n0\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 9);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"3\r\nabc\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 13);
        assert_eq!(s.preview_size, 3);
        assert!(s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"3\r\nabc\r\n0; ieof\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithoutTrailer,
            4,
            b"4\r\nabcd\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 14);
        assert_eq!(s.preview_size, 4);
        assert!(s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0; ieof\r\n\r\n");

        headers.clear();
        let s = push_preview_data(
            &mut headers,
            HttpBodyType::ChunkedWithTrailer,
            4,
            b"4\r\nabcd\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(s.consume_size, 9);
        assert_eq!(s.preview_size, 4);
        assert!(!s.preview_eof);
        assert_eq!(s.chunked_next_size, 0);
        assert_eq!(headers.as_slice(), b"4\r\nabcd\r\n0\r\n\r\n");
    }
}
