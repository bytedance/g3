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

use std::sync::Arc;

use anyhow::{anyhow, Context};
use serde_json::Value;

use g3_types::net::Host;
use g3_types::route::HostMatch;

use crate::JsonMapCallback;

fn add_host_matched_value<T: JsonMapCallback>(
    obj: &mut HostMatch<Arc<T>>,
    value: &Value,
    mut target: T,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Value::Object(map) = value {
        let mut exact_ip_vs = vec![];
        let mut exact_domain_vs = vec![];
        let mut child_domain_vs = vec![];
        let mut set_default = false;

        let mut add_exact_host_match_value = |v: &Value| -> anyhow::Result<()> {
            match crate::value::as_host(v)? {
                Host::Ip(ip) => exact_ip_vs.push(ip),
                Host::Domain(domain) => exact_domain_vs.push(domain),
            }
            Ok(())
        };

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "set_default" => {
                    set_default = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                }
                "exact_match" => {
                    if let Value::Array(seq) = v {
                        for (i, v) in seq.iter().enumerate() {
                            add_exact_host_match_value(v)
                                .context(format!("invalid host string value for {k}#{i}"))?;
                        }
                    } else {
                        add_exact_host_match_value(v)
                            .context(format!("invalid host string value for key {k}"))?;
                    }
                }
                "child_match" => {
                    if let Value::Array(seq) = v {
                        for (i, v) in seq.iter().enumerate() {
                            let domain = crate::value::as_domain(v)
                                .context(format!("invalid domain string value for {k}#{i}"))?;
                            child_domain_vs.push(domain);
                        }
                    } else {
                        let domain = crate::value::as_domain(v)
                            .context(format!("invalid domain string value for key {k}"))?;
                        child_domain_vs.push(domain);
                    }
                }
                normalized_key => target
                    .parse_kv(normalized_key, v)
                    .context(format!("failed to parse {type_name} value for key {k}"))?,
            }
        }

        target
            .check()
            .context(format!("{type_name} final check failed"))?;

        let t = Arc::new(target);
        let mut auto_default = true;
        for ip in exact_ip_vs {
            if obj.add_exact_ip(ip, Arc::clone(&t)).is_some() {
                return Err(anyhow!("duplicate {type_name} value for host ip {ip}"));
            }
            auto_default = false;
        }
        for domain in &exact_domain_vs {
            if obj
                .add_exact_domain(domain.to_string(), Arc::clone(&t))
                .is_some()
            {
                return Err(anyhow!(
                    "duplicate {type_name} value for host domain {domain}"
                ));
            }
            auto_default = false;
        }
        for domain in &child_domain_vs {
            if obj.add_child_domain(domain, Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for child domain {domain}"
                ));
            }
            auto_default = false;
        }
        if (set_default || auto_default) && obj.set_default(t).is_some() {
            return Err(anyhow!("a default {type_name} value has already been set"));
        }

        Ok(())
    } else {
        Err(anyhow!(
            "json type for 'host matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_host_matched_obj<T>(value: &Value) -> anyhow::Result<HostMatch<Arc<T>>>
where
    T: Default + JsonMapCallback,
{
    let mut obj = HostMatch::<Arc<T>>::default();

    if let Value::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_host_matched_value(&mut obj, v, target).context(format!(
                "invalid host matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_host_matched_value(&mut obj, value, target)
            .context(format!("invalid host matched {type_name} value"))?;
    }

    Ok(obj)
}
