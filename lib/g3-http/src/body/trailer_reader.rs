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
use std::io;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{ready, Context, Poll};

use http::HeaderName;
use thiserror::Error;
use tokio::io::AsyncBufRead;

use g3_types::net::{HttpHeaderMap, HttpHeaderValue};

use crate::{HttpHeaderLine, HttpLineParseError};

#[derive(Debug, Error)]
pub enum TrailerReadError {
    #[error("read error: {0:?}")]
    ReadError(#[from] io::Error),
    #[error("read closed")]
    ReadClosed,
    #[error("invalid header line: {0}")]
    InvalidHeaderLine(#[from] HttpLineParseError),
    #[error("trailer header too large")]
    HeaderTooLarge,
}

struct TrailerReaderInternal {
    trailer_max_size: usize,
    cached_line: Vec<u8>,
    headers: HttpHeaderMap,
    header_size: usize,
    active: bool,
}

impl TrailerReaderInternal {
    fn new(trailer_max_size: usize) -> Self {
        TrailerReaderInternal {
            trailer_max_size,
            cached_line: Vec::with_capacity(32),
            headers: HttpHeaderMap::default(),
            header_size: 0,
            active: false,
        }
    }

    #[inline]
    fn is_active(&self) -> bool {
        self.active
    }

    fn reset_active(&mut self) {
        self.active = false;
    }

    fn poll_read<R>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
    ) -> Poll<Result<HttpHeaderMap, TrailerReadError>>
    where
        R: AsyncBufRead + Unpin,
    {
        loop {
            let buf = ready!(reader.as_mut().poll_fill_buf(cx))?;
            self.active = true;
            if buf.is_empty() {
                return Poll::Ready(Err(TrailerReadError::ReadClosed));
            }
            let Some(p) = memchr::memchr(b'\n', buf) else {
                let len = buf.len();
                self.header_size += len;
                if self.header_size > self.trailer_max_size {
                    return Poll::Ready(Err(TrailerReadError::HeaderTooLarge));
                }
                self.cached_line.extend_from_slice(buf);
                reader.as_mut().consume(len);
                continue;
            };

            self.header_size += p + 1;
            if self.header_size > self.trailer_max_size {
                return Poll::Ready(Err(TrailerReadError::HeaderTooLarge));
            }
            self.cached_line.extend_from_slice(&buf[0..=p]);
            reader.as_mut().consume(p + 1);

            if self.cached_line[0] == b'\n'
                || (self.cached_line[0] == b'\r' && self.cached_line[1] == b'\n')
            {
                let headers = std::mem::take(&mut self.headers);
                return Poll::Ready(Ok(headers));
            }

            let header = HttpHeaderLine::parse(&self.cached_line)?;
            let name = HeaderName::from_str(header.name).map_err(|_| {
                TrailerReadError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderName)
            })?;
            let value = HttpHeaderValue::from_str(header.value).map_err(|_| {
                TrailerReadError::InvalidHeaderLine(HttpLineParseError::InvalidHeaderValue)
            })?;
            self.headers.append(name, value);
        }
    }
}

pub struct TrailerReader<'a, R> {
    reader: &'a mut R,
    internal: TrailerReaderInternal,
}

impl<'a, R> TrailerReader<'a, R> {
    pub fn new(reader: &'a mut R, trailer_max_size: usize) -> Self {
        TrailerReader {
            reader,
            internal: TrailerReaderInternal::new(trailer_max_size),
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.internal.is_active()
    }

    pub fn reset_active(&mut self) {
        self.internal.reset_active()
    }
}

impl<'a, R> Future for TrailerReader<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = Result<HttpHeaderMap, TrailerReadError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.internal.poll_read(cx, Pin::new(&mut me.reader))
    }
}
