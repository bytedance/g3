/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

use crate::metrics::{ParseError, chars_allowed_in_opentsdb};

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct MetricTagName(SmolStr);

impl MetricTagName {
    /// # Safety
    /// The characters in `s` is not checked
    pub const unsafe fn new_static_unchecked(s: &'static str) -> Self {
        MetricTagName(SmolStr::new_static(s))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn parse_buf(buf: &[u8]) -> Result<Self, ParseError> {
        let name = std::str::from_utf8(buf)?;
        MetricTagName::from_str(name)
    }
}

impl AsRef<str> for MetricTagName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for MetricTagName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricTagName(s.into()))
    }
}

impl fmt::Display for MetricTagName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_metrics_name() {
        assert_eq!(MetricTagName::from_str("abc-1").unwrap().as_str(), "abc-1");

        assert!(MetricTagName::from_str("a=b").is_err());
    }
}
