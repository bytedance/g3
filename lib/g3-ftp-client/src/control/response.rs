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

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::LimitedBufReadExt;

use super::FtpControlChannel;
use crate::error::FtpRawResponseError;

#[derive(Debug)]
pub(super) enum FtpRawResponse {
    SingleLine(u16, String),
    MultiLine(u16, Vec<String>),
}

macro_rules! char_to_u16 {
    ($c:expr) => {
        ($c - b'0') as u16
    };
}

impl FtpRawResponse {
    pub(super) fn parse_single_line(line: &[u8]) -> Result<Self, FtpRawResponseError> {
        let code = char_to_u16!(line[0]) * 100 + char_to_u16!(line[1]) * 10 + char_to_u16!(line[2]);
        if !(100..600).contains(&code) {
            return Err(FtpRawResponseError::InvalidReplyCode(code));
        }
        let msg =
            std::str::from_utf8(&line[4..]).map_err(|_| FtpRawResponseError::LineIsNotUtf8)?;
        Ok(FtpRawResponse::SingleLine(code, msg.trim_end().to_string()))
    }

    pub(super) fn get_multi_line_parser(
        line: &[u8],
        max_lines: usize,
    ) -> Result<FtpMultiLineReplyParser, FtpRawResponseError> {
        let code = char_to_u16!(line[0]) * 100 + char_to_u16!(line[1]) * 10 + char_to_u16!(line[2]);
        if !(100..600).contains(&code) {
            return Err(FtpRawResponseError::InvalidReplyCode(code));
        }
        let end_prefix = [line[0], line[1], line[2], b' '];
        let mut lines = Vec::<String>::with_capacity(max_lines);
        let msg =
            std::str::from_utf8(&line[4..]).map_err(|_| FtpRawResponseError::LineIsNotUtf8)?;
        lines.push(msg.trim_end().to_string());
        Ok(FtpMultiLineReplyParser {
            code,
            end_prefix,
            lines,
        })
    }

    pub(super) fn code(&self) -> u16 {
        match self {
            FtpRawResponse::SingleLine(code, _) => *code,
            FtpRawResponse::MultiLine(code, _) => *code,
        }
    }

    pub(super) fn line_trimmed(&self) -> Option<&str> {
        match self {
            FtpRawResponse::SingleLine(_, line) => Some(line.as_str().trim()),
            FtpRawResponse::MultiLine(_, _) => None,
        }
    }

    pub(super) fn lines(&self) -> Option<&[String]> {
        match self {
            FtpRawResponse::SingleLine(_, _) => None,
            FtpRawResponse::MultiLine(_, lines) => Some(lines),
        }
    }

    pub(super) fn parse_pasv_227_reply(&self) -> Option<SocketAddr> {
        let line = match self {
            FtpRawResponse::SingleLine(_, line) => line,
            FtpRawResponse::MultiLine(_, _) => return None,
        };

        if let Some(p_start) = memchr::memchr(b'(', line.as_bytes()) {
            if let Some(p_end) = memchr::memchr(b')', &line.as_bytes()[p_start..]) {
                let p_end = p_end + p_start;

                let a: Vec<&str> = line[p_start + 1..p_end].split(',').collect();
                if a.len() != 6 {
                    return None;
                }

                let h1 = u8::from_str(a[0]).ok()?;
                let h2 = u8::from_str(a[1]).ok()?;
                let h3 = u8::from_str(a[2]).ok()?;
                let h4 = u8::from_str(a[3]).ok()?;
                let p1 = u8::from_str(a[4]).ok()?;
                let p2 = u8::from_str(a[5]).ok()?;

                let ip = IpAddr::V4(Ipv4Addr::new(h1, h2, h3, h4));
                let port = ((p1 as u16) << 8) + (p2 as u16);
                return Some(SocketAddr::new(ip, port));
            }
        }

        None
    }

    pub(super) fn parse_epsv_229_reply(&self) -> Option<u16> {
        let line = match self {
            FtpRawResponse::SingleLine(_, line) => line,
            FtpRawResponse::MultiLine(_, _) => return None,
        };

        if let Some(p_start) = memchr::memchr(b'(', line.as_bytes()) {
            if let Some(p_end) = memchr::memchr(b')', &line.as_bytes()[p_start..]) {
                let p_end = p_end + p_start;

                if !line[p_start + 1..p_end].starts_with("|||") {
                    return None;
                }
                if p_end - 1 <= p_start + 4 {
                    return None;
                }
                if line.as_bytes()[p_end - 1] != b'|' {
                    return None;
                }
                let port = u16::from_str(&line[p_start + 4..p_end - 1]).ok()?;
                return Some(port);
            }
        }

        None
    }

    pub(super) fn parse_spsv_227_reply(&self) -> Option<String> {
        let line = match self {
            FtpRawResponse::SingleLine(_, line) => line,
            FtpRawResponse::MultiLine(_, _) => return None,
        };

        if let Some(p_start) = memchr::memchr(b'(', line.as_bytes()) {
            if let Some(p_end) = memchr::memchr(b')', &line.as_bytes()[p_start..]) {
                let identifier = line[p_start + 1..p_end].to_string();
                return Some(identifier);
            }
        }
        // pure-ftpd has removed it's SPSV support in commit
        // https://github.com/jedisct1/pure-ftpd/commit/4828633d9cb42cd77d764e7d1cb3d0c04c5df001

        None
    }
}

pub(super) struct FtpMultiLineReplyParser {
    code: u16,
    end_prefix: [u8; 4],
    lines: Vec<String>,
}

impl FtpMultiLineReplyParser {
    pub(super) fn feed_line(&mut self, line: &[u8]) -> Result<bool, FtpRawResponseError> {
        if line.starts_with(&self.end_prefix) {
            let msg =
                std::str::from_utf8(&line[4..]).map_err(|_| FtpRawResponseError::LineIsNotUtf8)?;
            self.lines.push(msg.trim_end().to_string());
            Ok(true)
        } else {
            let msg = std::str::from_utf8(line).map_err(|_| FtpRawResponseError::LineIsNotUtf8)?;
            // do not trim whitespace at beginning
            self.lines.push(msg.trim_end().to_string());
            Ok(false)
        }
    }

    pub(super) fn finish(self) -> FtpRawResponse {
        FtpRawResponse::MultiLine(self.code, self.lines)
    }
}

impl<T> FtpControlChannel<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    async fn read_first_line(&mut self, buf: &mut Vec<u8>) -> Result<(), FtpRawResponseError> {
        buf.clear();

        let (found, len) = self
            .stream
            .limited_read_until(b'\n', self.config.max_line_len, buf)
            .await
            .map_err(FtpRawResponseError::ReadFailed)?;
        match len {
            0 => Err(FtpRawResponseError::ConnectionClosed),
            1 | 2 | 3 | 4 => {
                // at least <code>\n

                #[cfg(feature = "log-raw-io")]
                crate::debug::log_rsp(unsafe { std::str::from_utf8_unchecked(buf).trim_end() });

                Err(FtpRawResponseError::InvalidLineFormat)
            }
            _ => {
                #[cfg(feature = "log-raw-io")]
                crate::debug::log_rsp(unsafe { std::str::from_utf8_unchecked(buf).trim_end() });

                if !found {
                    Err(FtpRawResponseError::LineTooLong)
                } else {
                    Ok(())
                }
            }
        }
    }

    async fn read_extra_line(&mut self, buf: &mut Vec<u8>) -> Result<(), FtpRawResponseError> {
        buf.clear();

        let (found, len) = self
            .stream
            .limited_read_until(b'\n', self.config.max_line_len, buf)
            .await
            .map_err(FtpRawResponseError::ReadFailed)?;
        match len {
            0 => Err(FtpRawResponseError::ConnectionClosed),
            1 => {
                // at least "\n"

                #[cfg(feature = "log-raw-io")]
                crate::debug::log_rsp(unsafe { std::str::from_utf8_unchecked(buf).trim_end() });

                Err(FtpRawResponseError::InvalidLineFormat)
            }
            _ => {
                #[cfg(feature = "log-raw-io")]
                crate::debug::log_rsp(unsafe { std::str::from_utf8_unchecked(buf).trim_end() });

                if !found {
                    Err(FtpRawResponseError::LineTooLong)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(super) async fn read_raw_response(
        &mut self,
    ) -> Result<FtpRawResponse, FtpRawResponseError> {
        let mut buf = Vec::<u8>::with_capacity(self.config.max_line_len);
        self.read_first_line(&mut buf).await?;

        match buf[3] {
            b' ' => FtpRawResponse::parse_single_line(&buf),
            b'-' => {
                let mut ml_parser =
                    FtpRawResponse::get_multi_line_parser(&buf, self.config.max_multi_lines)?;
                for _i in 0..self.config.max_multi_lines {
                    self.read_extra_line(&mut buf).await?;
                    let end = ml_parser.feed_line(&buf)?;
                    if end {
                        return Ok(ml_parser.finish());
                    }
                }
                Err(FtpRawResponseError::TooManyLines)
            }
            _ => Err(FtpRawResponseError::InvalidLineFormat),
        }
    }

    pub(super) async fn timed_read_raw_response(
        &mut self,
        stage: &'static str,
    ) -> Result<FtpRawResponse, FtpRawResponseError> {
        match tokio::time::timeout(self.config.command_timeout, self.read_raw_response()).await {
            Ok(r) => r,
            Err(_) => Err(FtpRawResponseError::ReadResponseTimedOut(stage)),
        }
    }
}
