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

use std::str::FromStr;

use anyhow::{anyhow, Context};
use serde_json::Value;

use g3_types::net::Host;
use g3_types::resolve::{PickStrategy, QueryStrategy, ResolveRedirectionBuilder, ResolveStrategy};

const RESOLVE_REDIRECTION_NODE_KEY_EXACT: &str = "exact";
const RESOLVE_REDIRECTION_NODE_KEY_PARENT: &str = "parent";
const RESOLVE_REDIRECTION_NODE_KEY_TO: &str = "to";

pub fn as_query_strategy(v: &Value) -> anyhow::Result<QueryStrategy> {
    match v {
        Value::String(s) => {
            QueryStrategy::from_str(s).map_err(|_| anyhow!("invalid query strategy string"))
        }
        _ => Err(anyhow!("invalid json value type for query strategy")),
    }
}

pub fn as_pick_strategy(v: &Value) -> anyhow::Result<PickStrategy> {
    match v {
        Value::String(s) => {
            PickStrategy::from_str(s).map_err(|_| anyhow!("invalid pick strategy string"))
        }
        _ => Err(anyhow!("invalid json value type for pick strategy")),
    }
}

pub fn as_resolve_strategy(v: &Value) -> anyhow::Result<ResolveStrategy> {
    let mut config = ResolveStrategy::default();

    match v {
        Value::String(_) => {
            config.query = as_query_strategy(v)?;
            Ok(config)
        }
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "query" => {
                        config.query = as_query_strategy(v)?;
                    }
                    "pick" => {
                        config.pick = as_pick_strategy(v)?;
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                };
            }
            Ok(config)
        }
        _ => Err(anyhow!("invalid json value type for resolve strategy")),
    }
}

fn add_exact_redirection_record(
    config: &mut ResolveRedirectionBuilder,
    domain: String,
    v: &Value,
) -> anyhow::Result<()> {
    match v {
        Value::String(_) => {
            match crate::value::as_host(v).context(format!(
                "invalid resolve redirect host value for domain {domain}",
            ))? {
                Host::Ip(ip) => config.insert_exact(domain, vec![ip]),
                Host::Domain(alias) => config.insert_exact_alias(domain, alias),
            }
            Ok(())
        }
        Value::Array(seq) => {
            let mut ips = Vec::with_capacity(seq.len());
            for (i, v) in seq.iter().enumerate() {
                let ip = crate::value::as_ipaddr(v)
                    .context(format!("invalid ip address value for domain {domain}#{i}",))?;
                ips.push(ip);
            }
            config.insert_exact(domain, ips);
            Ok(())
        }
        _ => Err(anyhow!(
            "invalid value type for resolve redirection value of domain {domain}",
        )),
    }
}

fn add_parent_redirection_record(
    config: &mut ResolveRedirectionBuilder,
    parent_domain: String,
    v: &Value,
) -> anyhow::Result<()> {
    let to_domain = crate::value::as_domain(v)
        .context("the value should be a domain for parent domain replace")?;
    config.insert_parent(parent_domain, to_domain);
    Ok(())
}

pub fn as_resolve_redirection_builder(v: &Value) -> anyhow::Result<ResolveRedirectionBuilder> {
    let mut config = ResolveRedirectionBuilder::default();

    match v {
        Value::Object(map) => {
            for (k, v) in map.iter() {
                let domain = idna::domain_to_ascii(k)
                    .map_err(|e| anyhow!("invalid domain to redirect({k}): {e}"))?;
                add_exact_redirection_record(&mut config, domain, v)?;
            }
            Ok(config)
        }
        Value::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Value::Object(map) = v {
                    let to_v = crate::map_get_required(map, RESOLVE_REDIRECTION_NODE_KEY_TO)?;
                    if let Ok(exact) =
                        crate::get_required_str(map, RESOLVE_REDIRECTION_NODE_KEY_EXACT)
                    {
                        let domain = idna::domain_to_ascii(exact)
                            .map_err(|e| anyhow!("invalid exact domain in element #{i}: {e}"))?;

                        add_exact_redirection_record(&mut config, domain, to_v).context(
                            format!("invalid exact domain replacement value for element #{i}"),
                        )?;
                    } else if let Ok(parent) =
                        crate::get_required_str(map, RESOLVE_REDIRECTION_NODE_KEY_PARENT)
                    {
                        let parent_domain = idna::domain_to_ascii(parent)
                            .map_err(|e| anyhow!("invalid parent domain in element #{i}: {e}"))?;

                        add_parent_redirection_record(&mut config, parent_domain, to_v).context(
                            format!("invalid parent domain replacement value for element #{i}"),
                        )?;
                    } else {
                        return Err(anyhow!("no exact or parent domain set in element #{i}"));
                    }
                } else {
                    return Err(anyhow!("invalid map value for element #{i}"));
                }
            }
            Ok(config)
        }
        _ => Err(anyhow!("invalid json value type for resolve redirection")),
    }
}
