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

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, Error)]
pub enum RecvLineError {
    #[error("io error: {0:?}")]
    IoError(#[from] io::Error),
    #[error("io closed")]
    IoClosed,
    #[error("line too long")]
    LineTooLong,
}

pub struct LineRecvBuf<const MAX_LINE_SIZE: usize> {
    length: usize,
    line_start: usize,
    line_end: usize,
    buf: [u8; MAX_LINE_SIZE],
}

impl<const MAX_LINE_SIZE: usize> Default for LineRecvBuf<MAX_LINE_SIZE> {
    fn default() -> Self {
        LineRecvBuf {
            length: 0,
            line_start: 0,
            line_end: 0,
            buf: [0u8; MAX_LINE_SIZE],
        }
    }
}

impl<const MAX_LINE_SIZE: usize> LineRecvBuf<MAX_LINE_SIZE> {
    pub async fn read_line<'a, R>(&'a mut self, reader: &mut R) -> Result<&[u8], RecvLineError>
    where
        R: AsyncRead + Unpin,
    {
        self.read_line_size(reader).await?;
        Ok(&self.buf[self.line_start..self.line_end])
    }

    async fn read_line_size<R>(&mut self, reader: &mut R) -> Result<(), RecvLineError>
    where
        R: AsyncRead + Unpin,
    {
        if let Some(end) = self.get_line() {
            return Ok(end);
        }
        loop {
            let mut unfilled = &mut self.buf[self.length..];
            if unfilled.is_empty() {
                return self.get_line().ok_or(RecvLineError::LineTooLong);
            }
            let nr = reader.read_buf(&mut unfilled).await?;
            if nr == 0 {
                return self.get_line().ok_or(RecvLineError::IoClosed);
            }
            self.length += nr;
            if let Some(line) = self.get_line() {
                return Ok(line);
            }
        }
    }

    fn get_line(&mut self) -> Option<()> {
        if self.line_end > self.line_start {
            return Some(());
        }
        if self.line_start >= self.length {
            return None;
        }
        match memchr::memchr(b'\n', &self.buf[self.line_start..self.length]) {
            Some(p) => {
                self.line_end = self.line_start + p + 1;
                Some(())
            }
            None => None,
        }
    }

    pub fn buffered_line(&mut self) -> Option<&[u8]> {
        self.get_line()?;
        Some(&self.buf[self.line_start..self.line_end])
    }

    pub fn consume_line(&mut self) {
        self.line_start = self.line_end;
        if self.get_line().is_some() {
            return;
        }
        if self.line_start < self.length {
            self.buf.copy_within(self.line_start..self.length, 0);
            self.length -= self.line_start;
        } else {
            self.length = 0;
        }
        self.line_start = 0;
        self.line_end = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn single_line() {
        let data = b"123 test line\r\n";

        let stream = tokio_stream::iter(vec![io::Result::Ok(data.as_slice())]);
        let mut reader = StreamReader::new(stream);

        let mut b: LineRecvBuf<512> = LineRecvBuf::default();
        let line = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line, data);
        b.consume_line();
        assert!(b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    async fn multiple_line() {
        let data = b"123 test line\r\n456 second line\r\n789";

        let stream = tokio_stream::iter(vec![io::Result::Ok(data.as_slice())]);
        let mut reader = StreamReader::new(stream);

        let mut b: LineRecvBuf<512> = LineRecvBuf::default();
        let line1 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line1, b"123 test line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let line2 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line2, b"456 second line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    async fn multiple_line2() {
        let data1 = b"123 test line\r\n";
        let data2 = b"456 second ";
        let data3 = b"line\r\n789";

        let stream = tokio_stream::iter(vec![
            io::Result::Ok(data1.as_slice()),
            io::Result::Ok(data2.as_slice()),
            io::Result::Ok(data3.as_slice()),
        ]);
        let mut reader = StreamReader::new(stream);

        let mut b: LineRecvBuf<512> = LineRecvBuf::default();
        let line1 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line1, b"123 test line\r\n");
        b.consume_line();
        assert!(b.is_empty());

        let line2 = b.read_line(&mut reader).await.unwrap();
        assert_eq!(line2, b"456 second line\r\n");
        b.consume_line();
        assert!(!b.is_empty());

        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::IoClosed)));
    }

    #[tokio::test]
    async fn too_long_line() {
        let data = b"123 test line\r\n";

        let stream = tokio_stream::iter(vec![io::Result::Ok(data.as_slice())]);
        let mut reader = StreamReader::new(stream);

        let mut b: LineRecvBuf<12> = LineRecvBuf::default();
        let r = b.read_line(&mut reader).await;
        assert!(matches!(r, Err(RecvLineError::LineTooLong)));
    }
}
