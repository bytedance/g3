/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;

#[derive(Clone, Copy)]
pub enum ResolveQueryType {
    A,
    Aaaa,
}

impl ResolveQueryType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ResolveQueryType::A => "A",
            ResolveQueryType::Aaaa => "AAAA",
        }
    }
}

impl AsRef<str> for ResolveQueryType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ResolveQueryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
