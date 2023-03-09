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

use std::collections::BTreeMap;
use std::str::FromStr;

use super::{chars_allowed_in_opentsdb, ParseError};

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct MetricsTagName(String);

#[derive(Debug, Eq, PartialEq)]
pub struct MetricsTagValue(String);

pub type StaticMetricsTags = BTreeMap<MetricsTagName, MetricsTagValue>;

impl MetricsTagName {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for MetricsTagName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricsTagName(s.to_string()))
    }
}

impl MetricsTagValue {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for MetricsTagValue {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricsTagValue(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_metrics_name() {
        assert_eq!(
            MetricsTagName::from_str("abc-1").unwrap(),
            MetricsTagName("abc-1".to_string())
        );

        assert!(MetricsTagName::from_str("a=b").is_err());
    }

    #[test]
    fn t_metrics_value() {
        assert_eq!(
            MetricsTagValue::from_str("abc-1").unwrap(),
            MetricsTagValue("abc-1".to_string())
        );

        assert!(MetricsTagValue::from_str("a=b").is_err());
    }
}
