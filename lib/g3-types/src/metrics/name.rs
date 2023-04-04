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

use std::fmt;
use std::str::FromStr;

use super::{chars_allowed_in_opentsdb, ParseError};

#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct MetricsName(String);

impl MetricsName {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Get a MetricsName from a String value
    ///
    /// # Safety
    ///
    /// Call this only if you need not use the value in metrics
    pub unsafe fn from_unchecked(name: String) -> Self {
        MetricsName(name)
    }

    /// Get a MetricsName from a str value
    ///
    /// # Safety
    ///
    /// Call this only if you need not use the value in metrics
    pub unsafe fn from_str_unchecked(name: &str) -> Self {
        Self::from_unchecked(name.to_string())
    }
}

impl FromStr for MetricsName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricsName(s.to_string()))
    }
}

impl fmt::Display for MetricsName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> Default for &'a MetricsName {
    fn default() -> &'a MetricsName {
        static VALUE: MetricsName = MetricsName(String::new());
        &VALUE
    }
}
