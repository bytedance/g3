/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

use crate::types::MetricRecord;

mod line;
use line::{LineParser, LineValueIter};

#[derive(Debug, Error)]
pub(super) enum StatsdParseError {
    #[error("no name field")]
    NoName,
    #[error("invalid name field: {0}")]
    InvalidName(anyhow::Error),
    #[error("no value field")]
    NoValue,
    #[error("invalid value field: {0}")]
    InvalidValue(anyhow::Error),
    #[error("no type field")]
    NoType,
    #[error("unsupported type")]
    UnsupportedType,
    #[error("invalid tag value field: {0}")]
    InvalidTagValue(anyhow::Error),
}

pub(super) struct StatsdRecordVisitor<'a> {
    buf: &'a [u8],
    offset: usize,
    line_value_iter: Option<LineValueIter<'a>>,
}

impl<'a> StatsdRecordVisitor<'a> {
    pub(super) fn new(buf: &'a [u8]) -> Self {
        StatsdRecordVisitor {
            buf,
            offset: 0,
            line_value_iter: None,
        }
    }

    fn next_line(&mut self) -> Option<&'a [u8]> {
        if self.offset >= self.buf.len() {
            return None;
        }

        let left = &self.buf[self.offset..];
        match memchr::memchr(b'\n', left) {
            Some(p) => {
                self.offset += p + 1;
                Some(&left[..p])
            }
            None => {
                self.offset = self.buf.len();
                Some(left)
            }
        }
    }
}

impl Iterator for StatsdRecordVisitor<'_> {
    type Item = Result<MetricRecord, StatsdParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(mut line_iter) = self.line_value_iter.take()
                && let Some(r) = line_iter.next()
            {
                self.line_value_iter = Some(line_iter);
                return Some(r);
            }

            let line = self.next_line()?;
            if line.is_empty() {
                continue;
            }

            match LineParser::new(line).parse() {
                Ok(line_iter) => self.line_value_iter = Some(line_iter),
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MetricType, MetricValue};

    #[test]
    fn etsy_statsd() {
        let buf = b"gorets:1|c\n\ngaugor:333|g\n";

        let mut iter = StatsdRecordVisitor::new(buf);
        let r1 = iter.next().unwrap().unwrap();
        assert_eq!(r1.r#type, MetricType::Counter);
        assert_eq!(r1.value, MetricValue::Unsigned(1));

        let r2 = iter.next().unwrap().unwrap();
        assert_eq!(r2.r#type, MetricType::Gauge);
        assert_eq!(r2.value, MetricValue::Unsigned(333));

        assert!(iter.next().is_none());
    }
}
