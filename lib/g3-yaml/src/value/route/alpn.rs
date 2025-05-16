/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;
use g3_types::route::AlpnMatch;

use crate::{YamlDocPosition, YamlMapCallback};

fn add_alpn_matched_value<T: YamlMapCallback>(
    obj: &mut AlpnMatch<Arc<T>>,
    value: &Yaml,
    mut target: T,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Yaml::Hash(map) = value {
        let mut protocol_vs = vec![];
        let mut set_default = false;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "protocol" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let protocol = crate::value::as_string(v)
                            .context(format!("invalid string value for {k}#{i}"))?;
                        protocol_vs.push(protocol);
                    }
                } else {
                    let protocol = crate::value::as_string(v)
                        .context(format!("invalid string value for {k}"))?;
                    protocol_vs.push(protocol);
                }
                Ok(())
            }
            normalized_key => target
                .parse_kv(normalized_key, v, doc)
                .context(format!("failed to parse {type_name} value for key {k}")),
        })?;

        target
            .check()
            .context(format!("{type_name} final check failed"))?;

        let t = Arc::new(target);
        let mut auto_default = true;
        for protocol in protocol_vs {
            if obj.add_protocol(protocol.clone(), Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for protocol {protocol}"
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
            "yaml type for 'alpn matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_alpn_matched_obj<T>(
    value: &Yaml,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<AlpnMatch<Arc<T>>>
where
    T: Default + YamlMapCallback,
{
    let mut obj = AlpnMatch::<Arc<T>>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_alpn_matched_value(&mut obj, v, target, doc).context(format!(
                "invalid alpn matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_alpn_matched_value(&mut obj, value, target, doc)
            .context(format!("invalid alpn matched {type_name} value"))?;
    }

    Ok(obj)
}

fn add_alpn_matched_backend(obj: &mut AlpnMatch<NodeName>, value: &Yaml) -> anyhow::Result<()> {
    let mut protocol_vs = vec![];
    let mut set_default = false;
    let mut name = NodeName::default();

    if let Yaml::Hash(map) = value {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "protocol" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let protocol = crate::value::as_string(v)
                            .context(format!("invalid string value for {k}#{i}"))?;
                        protocol_vs.push(protocol);
                    }
                } else {
                    let protocol = crate::value::as_string(v)
                        .context(format!("invalid string value for {k}"))?;
                    protocol_vs.push(protocol);
                }
                Ok(())
            }
            "backend" => {
                name = crate::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
    } else {
        name = crate::value::as_metric_node_name(value)?;
    }

    let mut auto_default = true;
    for protocol in protocol_vs {
        if obj.add_protocol(protocol.clone(), name.clone()).is_some() {
            return Err(anyhow!("duplicate value for protocol {protocol}"));
        }
        auto_default = false;
    }
    if (set_default || auto_default) && obj.set_default(name).is_some() {
        return Err(anyhow!("a default value has already been set"));
    }
    Ok(())
}

pub fn as_alpn_matched_backends(value: &Yaml) -> anyhow::Result<AlpnMatch<NodeName>> {
    let mut obj = AlpnMatch::<NodeName>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            add_alpn_matched_backend(&mut obj, v)
                .context(format!("invalid alpn matched name value for element #{i}"))?;
        }
    } else {
        add_alpn_matched_backend(&mut obj, value).context("invalid alpn matched name value")?;
    }

    Ok(obj)
}
