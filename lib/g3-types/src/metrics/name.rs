/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

use super::{ParseError, chars_allowed_in_opentsdb};

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

    pub const fn new_static(name: &'static str) -> Self {
        NodeName(SmolStr::new_static(name))
    }
}

impl FromStr for NodeName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseError::UnexpectedEmpty);
        }
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
