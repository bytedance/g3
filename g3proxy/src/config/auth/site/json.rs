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

use anyhow::{anyhow, Context};
use serde_json::Value;

use super::UserSiteConfig;

impl UserSiteConfig {
    pub(crate) fn parse_json(v: &Value) -> anyhow::Result<Self> {
        if let Value::Object(map) = v {
            let mut config = UserSiteConfig::default();
            for (k, v) in map {
                config.set_json(k, v)?;
            }
            config.check()?;
            Ok(config)
        } else {
            Err(anyhow!("json value type for 'user site' should be 'map'"))
        }
    }

    fn set_json(&mut self, k: &str, v: &Value) -> anyhow::Result<()> {
        match g3_json::key::normalize(k).as_str() {
            "id" | "name" => {
                self.id = g3_json::value::as_metrics_name(v)
                    .context(format!("invalid metrics name value for key {k}"))?;
                Ok(())
            }
            "exact_match" => {
                if let Value::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let host = g3_json::value::as_host(v)
                            .context(format!("invalid host value for {k}#{i}"))?;
                        self.add_exact_host(host);
                    }
                } else {
                    let host = g3_json::value::as_host(v)
                        .context(format!("invalid host value(s) for key {k}"))?;
                    self.add_exact_host(host);
                }
                Ok(())
            }
            "subnet_match" => {
                if let Value::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let net = g3_json::value::as_ip_network(v)
                            .context(format!("invalid ip network value for {k}#{i}"))?;
                        self.subnet_match_ipaddr.insert(net);
                    }
                } else {
                    let net = g3_json::value::as_ip_network(v)
                        .context(format!("invalid ip network value(s) for key {k}"))?;
                    self.subnet_match_ipaddr.insert(net);
                }
                Ok(())
            }
            "child_match" => {
                if let Value::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let domain = g3_json::value::as_domain(v)
                            .context(format!("invalid domain value for {k}#{i}"))?;
                        self.child_match_domain.insert(domain);
                    }
                } else {
                    let domain = g3_json::value::as_domain(v)
                        .context(format!("invalid domain value(s) for key {k}"))?;
                    self.child_match_domain.insert(domain);
                }
                Ok(())
            }
            "emit_stats" => {
                self.emit_stats = g3_json::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "resolve_strategy" => {
                let strategy = g3_json::value::as_resolve_strategy(v)
                    .context(format!("invalid resolve strategy value for key {k}"))?;
                self.resolve_strategy = Some(strategy);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
