/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

pub(super) fn is_valid(s: &str) -> bool {
    let Some(s) = s.strip_prefix('<') else {
        return false;
    };
    let Some(s) = s.strip_suffix('>') else {
        return false;
    };

    for c in s.chars() {
        let c = c as u32;
        // 0-31 and >127 are not allowed
        // see https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.2
        if !(32..127).contains(&c) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_valid() {
        // Valid paths
        assert!(is_valid("<admin@company.net>"));
        assert!(is_valid("<>"));
        assert!(is_valid("<user.name@example.com>"));
        assert!(is_valid("<user+tag@example.com>"));
        assert!(is_valid("<user_name@example.com>"));
        assert!(is_valid("<user-name@example.com>"));
        assert!(is_valid("<@relay.com:user@example.com>"));
        assert!(is_valid("<@relay1.com,@relay2.com:user@example.com>"));

        // Missing opening bracket
        assert!(!is_valid("user@example.com>"));
        assert!(!is_valid("test@domain.org>"));

        // Missing closing bracket
        assert!(!is_valid("<user@example.com"));
        assert!(!is_valid("<test@domain.org"));

        // With control characters (0-31)
        assert!(!is_valid("<user\x01@example.com>"));
        assert!(!is_valid("<user\x1f@example.com>"));
        assert!(!is_valid("<user\x00@example.com>"));
        assert!(!is_valid("<user\t@example.com>"));
        assert!(!is_valid("<user\n@example.com>"));
        assert!(!is_valid("<user\r@example.com>"));

        // With characters > 127
        assert!(!is_valid("<user@exämple.com>"));
        assert!(!is_valid("<user@example.cöm>"));
        assert!(!is_valid("<üser@example.com>"));
        assert!(!is_valid("<user@测试.com>"));

        // Boundary characters
        assert!(is_valid("< @example.com>")); // space (32) - valid
        assert!(is_valid("<~@example.com>")); // tilde (126) - valid
        assert!(!is_valid("<\x1f@example.com>")); // 31 - invalid
        assert!(!is_valid("<\x7f@example.com>")); // 127 - invalid
    }
}
