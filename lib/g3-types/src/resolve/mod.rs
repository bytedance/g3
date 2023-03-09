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
