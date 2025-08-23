/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

use crate::metrics::{ParseError, chars_allowed_in_opentsdb};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct MetricTagValue(SmolStr);

impl MetricTagValue {
    pub const EMPTY: MetricTagValue = MetricTagValue(SmolStr::new_static(""));

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn parse_buf(buf: &[u8]) -> Result<Self, ParseError> {
        let value = std::str::from_utf8(buf)?;
        MetricTagValue::from_str(value)
    }
}

impl AsRef<str> for MetricTagValue {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for MetricTagValue {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricTagValue(s.into()))
    }
}

impl fmt::Display for MetricTagValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_metrics_value() {
        assert_eq!(MetricTagValue::from_str("abc-1").unwrap().as_str(), "abc-1");

        assert!(MetricTagValue::from_str("a=b").is_err());
    }
}
