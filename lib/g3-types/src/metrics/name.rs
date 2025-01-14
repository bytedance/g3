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

use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

use super::{chars_allowed_in_opentsdb, ParseError};

#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct NodeName(SmolStr);

impl NodeName {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = SmolStr::default();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Get a MetricsName from a str value
    ///
    /// # Safety
    ///
    /// Call this only if you need not use the value in metrics
    pub unsafe fn new_unchecked<T: AsRef<str>>(name: T) -> Self {
        NodeName(SmolStr::new(name))
    }
}

impl FromStr for NodeName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(NodeName(s.into()))
    }
}

impl AsRef<str> for NodeName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NodeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl<'a> Default for &'a NodeName {
    fn default() -> &'a NodeName {
        static VALUE: NodeName = NodeName(SmolStr::new_static(""));
        &VALUE
    }
}

impl Borrow<str> for NodeName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
