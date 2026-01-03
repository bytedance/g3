/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::net::IpAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use foldhash::HashMap;
use ip_network_table::IpNetworkTable;

use g3_types::auth::FactsMatchValue;

use super::BaseUserGroup;
use crate::auth::{User, UserType};
use crate::config::auth::FactsUserGroupConfig;

pub(crate) struct FactsUserGroup {
    base: BaseUserGroup<FactsUserGroupConfig>,
    match_table: ArcSwap<FactsMatchTable>,
}

impl FactsUserGroup {
    pub(super) fn base(&self) -> &BaseUserGroup<FactsUserGroupConfig> {
        &self.base
    }

    pub(super) fn clone_config(&self) -> FactsUserGroupConfig {
        (*self.base.config).clone()
    }

    fn new(base: BaseUserGroup<FactsUserGroupConfig>) -> Self {
        let match_table = FactsMatchTable::build(&base);
        FactsUserGroup {
            base,
            match_table: ArcSwap::new(Arc::new(match_table)),
        }
    }

    pub(super) fn rebuild_match_table(&self) {
        let match_table = FactsMatchTable::build(&self.base);
        self.match_table.store(Arc::new(match_table));
    }

    pub(super) async fn new_with_config(config: FactsUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = BaseUserGroup::new_with_config(config).await?;
        Ok(Arc::new(Self::new(base)))
    }

    pub(super) fn reload(&self, config: FactsUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = self.base.reload(config)?;
        Ok(Arc::new(Self::new(base)))
    }

    pub(crate) fn get_user_by_ip(&self, ip: IpAddr) -> Option<(Arc<User>, UserType)> {
        let match_table = self.match_table.load();
        if let Some(v) = match_table.get_user_by_ip(ip) {
            return Some(v);
        }

        self.base.get_anonymous_user()
    }
}

struct FactsMatchTable {
    exact_ip: HashMap<IpAddr, (Arc<User>, UserType)>,
    network: IpNetworkTable<(Arc<User>, UserType)>,
}

impl FactsMatchTable {
    fn build(base: &BaseUserGroup<FactsUserGroupConfig>) -> Self {
        let mut table = FactsMatchTable {
            exact_ip: Default::default(),
            network: IpNetworkTable::new(),
        };

        base.foreach_dynamic_user(|_, user| {
            table.add_user(user, UserType::Static);
        });

        base.foreach_static_user(|_, user| {
            table.add_user(user, UserType::Static);
        });

        table
    }

    fn add_user(&mut self, user: &Arc<User>, user_type: UserType) {
        for fact in user.match_facts() {
            match fact {
                FactsMatchValue::Ip(ip) => {
                    self.exact_ip.insert(*ip, (user.clone(), user_type));
                }
                FactsMatchValue::Network(net) => {
                    self.network.insert(*net, (user.clone(), user_type));
                }
            }
        }
    }

    fn get_user_by_ip(&self, ip: IpAddr) -> Option<(Arc<User>, UserType)> {
        if let Some((user, user_type)) = self.exact_ip.get(&ip) {
            return Some((user.clone(), *user_type));
        }

        if let Some((_, (user, user_type))) = self.network.longest_match(ip) {
            return Some((user.clone(), *user_type));
        }

        None
    }
}
