/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_dpi::{ProtocolInspectAction, ProtocolInspectPolicyBuilder};

mod child_domain;
mod exact_host;
mod network;

trait InspectRuleYamlParser {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()>;

    fn parse(&mut self, value: &Yaml) -> anyhow::Result<()> {
        if let Yaml::Hash(map) = value {
            crate::foreach_kv(map, |k, v| {
                let action = ProtocolInspectAction::from_str(k)
                    .map_err(|_| anyhow!("the key {k} is not a valid Action"))?;
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
            })
        } else {
            Err(anyhow!("invalid value type"))
        }
    }
}

fn as_protocol_inspect_action(value: &Yaml) -> anyhow::Result<ProtocolInspectAction> {
    if let Yaml::String(s) = value {
        ProtocolInspectAction::from_str(s)
            .map_err(|_| anyhow!("invalid protocol inspect action '{s}'"))
    } else {
        Err(anyhow!("invalid value type"))
    }
}

pub fn as_protocol_inspect_policy_builder(
    value: &Yaml,
) -> anyhow::Result<ProtocolInspectPolicyBuilder> {
    match value {
        Yaml::Hash(map) => {
            let mut builder = ProtocolInspectPolicyBuilder::default();
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "default" => {
                    let missed_action = as_protocol_inspect_action(v)
                        .context(format!("invalid protocol inspect action value for key {k}"))?;
                    builder.set_missed_action(missed_action);
                    Ok(())
                }
                "exact_match" | "exact" => {
                    let exact_rule = exact_host::as_exact_host_rule(v)
                        .context(format!("invalid exact host inspect rule value for key {k}"))?;
                    builder.exact = Some(exact_rule);
                    Ok(())
                }
                "child_match" | "child" => {
                    let child_builder = child_domain::as_child_domain_rule_builder(v).context(
                        format!("invalid child domain inspect rule value for key {k}"),
                    )?;
                    builder.child = Some(child_builder);
                    Ok(())
                }
                "subnet_match" | "subnet" => {
                    let subnet_builder = network::as_dst_subnet_rule_builder(v)
                        .context(format!("invalid subnet inspect rule value for key {k}"))?;
                    builder.subnet = Some(subnet_builder);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(builder)
        }
        _ => {
            let missed_action = as_protocol_inspect_action(value)?;
            Ok(ProtocolInspectPolicyBuilder::new(missed_action))
        }
    }
}
