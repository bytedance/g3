/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

pub fn normalize(raw: &str) -> String {
    raw.to_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t() {
        assert_eq!(normalize("Abc"), "abc");
        assert_eq!(normalize("ABC"), "abc");
        assert_eq!(normalize("A-B-C"), "a_b_c");
        assert_eq!(normalize("A-B_C"), "a_b_c");
    }
}
