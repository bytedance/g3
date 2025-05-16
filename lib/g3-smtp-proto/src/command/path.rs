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
