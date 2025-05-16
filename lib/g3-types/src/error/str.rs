/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct FoundInvalidChar {
    position: usize,
    value: char,
}

impl FoundInvalidChar {
    pub fn new(position: usize, value: char) -> Self {
        FoundInvalidChar { position, value }
    }
}

impl fmt::Display for FoundInvalidChar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "found invalid char {} at position {}",
            self.value.escape_default(),
            self.position
        )
    }
}

impl Error for FoundInvalidChar {}
