/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod redirect;
mod strategy;

pub use redirect::{ResolveRedirection, ResolveRedirectionBuilder, ResolveRedirectionValue};
pub use strategy::{PickStrategy, QueryStrategy, ResolveStrategy};

/// the input domain should be valid IDNA domain
pub fn reverse_idna_domain(domain: &str) -> String {
    let from = domain.strip_prefix('.').unwrap_or(domain);
    let mut domain = from.split('.').rev().collect::<Vec<&str>>().join(".");
    domain.push('.');
    domain
}

pub fn reverse_to_idna_domain(reversed: &str) -> String {
    let reversed = reversed.strip_suffix('.').unwrap_or(reversed);
    reversed.split('.').rev().collect::<Vec<&str>>().join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_domain() {
        assert_eq!(reverse_idna_domain("xjtu.edu.cn"), "cn.edu.xjtu.");
        assert_eq!(reverse_idna_domain(".xjtu.edu.cn"), "cn.edu.xjtu.");

        let reversed = reverse_idna_domain("www.xjtu.edu.cn");
        assert_eq!(reverse_to_idna_domain(&reversed), "www.xjtu.edu.cn");
    }
}
