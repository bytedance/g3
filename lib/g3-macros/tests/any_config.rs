/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use g3_macros::AnyConfig;

trait TestConfig {
    fn name(&self) -> &str;

    fn version(&self) -> usize;
    fn same_as(&self, other: &AnyTestConfig) -> bool;

    fn reload(&self);

    #[allow(unused)]
    async fn run(&self);
}

struct ConfigA {}

impl TestConfig for ConfigA {
    fn name(&self) -> &str {
        "A"
    }

    fn version(&self) -> usize {
        1
    }

    fn same_as(&self, _other: &AnyTestConfig) -> bool {
        false
    }

    fn reload(&self) {}

    async fn run(&self) {}
}

#[derive(AnyConfig)]
#[def_fn(name, &str)]
#[def_fn(version, usize)]
#[def_fn(same_as, &AnyTestConfig, bool)]
#[def_fn(reload)]
#[def_async_fn(run)]
pub(crate) enum AnyTestConfig {
    Variant1(ConfigA),
    // Variant 2
    Variant2(ConfigA),
}

#[test]
fn test_any() {
    let config = ConfigA {};
    let any_config = AnyTestConfig::Variant1(config);
    assert_eq!(any_config.name(), "A");
    assert_eq!(any_config.version(), 1);
    any_config.reload();

    let any_config2 = AnyTestConfig::Variant2(ConfigA {});
    assert!(!any_config.same_as(&any_config2));
}
