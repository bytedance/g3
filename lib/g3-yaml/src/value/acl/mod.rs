/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::acl::AclAction;

mod child_domain;
mod exact_host;
mod exact_port;
mod network;
mod proxy_request;
mod regex_domain;
mod regex_set;
mod user_agent;

pub(crate) use child_domain::as_child_domain_rule_builder;
pub(crate) use exact_host::as_exact_host_rule;
pub(crate) use network::as_dst_subnet_rule_builder;
pub(crate) use regex_domain::as_regex_domain_rule_builder;

pub use exact_port::as_exact_port_rule;
pub use network::{as_egress_network_rule_builder, as_ingress_network_rule_builder};
pub use proxy_request::as_proxy_request_rule;
pub use regex_set::as_regex_set_rule_builder;
pub use user_agent::as_user_agent_rule;

fn as_action(value: &Yaml) -> anyhow::Result<AclAction> {
    if let Yaml::String(s) = value {
        let action =
            AclAction::from_str(s).map_err(|_| anyhow!("invalid AclAction string value"))?;
        Ok(action)
    } else {
        Err(anyhow!(
            "the yaml value type for AclAction should be string"
        ))
    }
}

trait AclRuleYamlParser {
    fn get_default_found_action(&self) -> AclAction;
    fn set_missed_action(&mut self, action: AclAction);
    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()>;

    fn parse(&mut self, value: &Yaml) -> anyhow::Result<()> {
        match value {
            Yaml::Hash(map) => {
                crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                    "default" => {
                        let action = as_action(v).context(format!("invalid value for key {k}"))?;
                        self.set_missed_action(action);
                        Ok(())
                    }
                    _ => {
                        let action = AclAction::from_str(k)
                            .map_err(|_| anyhow!("the key {k} is not a valid AclAction"))?;
                        if let Yaml::Array(seq) = v {
                            for (i, v) in seq.iter().enumerate() {
                                self.add_rule_for_action(action, v)
                                    .context(format!("invalid value for {k}#{i}"))?;
                            }
                            Ok(())
                        } else {
                            self.add_rule_for_action(action, v)
                                .context(format!("invalid value for key {k}"))
                        }
                    }
                })?;
            }
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    self.add_rule_for_action(self.get_default_found_action(), v)
                        .context(format!("invalid value for element #{i}"))?;
                }
            }
            _ => {
                self.add_rule_for_action(self.get_default_found_action(), value)?;
            }
        }
        Ok(())
    }
}
