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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_invalid_char_operations() {
        let err = FoundInvalidChar::new(0, 'a');
        assert_eq!(err.position, 0);
        assert_eq!(err.value, 'a');

        let err = FoundInvalidChar::new(42, ';');
        assert_eq!(err.position, 42);
        assert_eq!(err.value, ';');

        let test_cases = vec![
            (0, 'a', "found invalid char a at position 0"),
            (5, '\x01', "found invalid char \\u{1} at position 5"),
            (10, '\x7F', "found invalid char \\u{7f} at position 10"),
            (15, ';', "found invalid char ; at position 15"),
            (20, ',', "found invalid char , at position 20"),
            (25, 'é', "found invalid char \\u{e9} at position 25"),
            (30, '中', "found invalid char \\u{4e2d} at position 30"),
        ];

        for (position, value, expected_msg) in test_cases {
            let err = FoundInvalidChar::new(position, value);
            let display_output = err.to_string();
            assert_eq!(display_output, expected_msg);
        }
    }
}
