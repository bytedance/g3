/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::num::NonZeroUsize;
use std::time::Duration;

use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

mod basic;
pub(crate) use basic::BasicUserGroupConfig;

mod facts;
pub(crate) use facts::FactsUserGroupConfig;

mod ldap;
pub(crate) use ldap::LdapUserGroupConfig;

pub(crate) const DEFAULT_CACHE_USER_COUNT: NonZeroUsize = NonZeroUsize::new(128).unwrap();
const DEFAULT_CACHE_EXPIRE_TIME: Duration = Duration::from_secs(300);

pub(crate) trait UserGroupConfig {
    fn basic_config(&self) -> &BasicUserGroupConfig;

    fn r#type(&self) -> &'static str;
}

#[derive(Clone, AnyConfig)]
#[def_fn(basic_config, &BasicUserGroupConfig)]
#[def_fn(r#type, &'static str)]
pub(crate) enum AnyUserGroupConfig {
    Basic(BasicUserGroupConfig),
    Facts(FactsUserGroupConfig),
    Ldap(LdapUserGroupConfig),
}

impl AnyUserGroupConfig {
    pub(crate) fn name(&self) -> &NodeName {
        self.basic_config().name()
    }

    pub(crate) fn position(&self) -> Option<YamlDocPosition> {
        self.basic_config().position()
    }
}
