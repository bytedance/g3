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

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use serde_json::Value;

use g3_socket::util::AddressFamily;

use super::BindSet;
use crate::config::escaper::direct_float::DirectFloatEscaperConfig;

async fn load_records_from_cache(cache_file: &Path) -> anyhow::Result<Value> {
    let contents = tokio::fs::read_to_string(cache_file).await.map_err(|e| {
        anyhow!(
            "failed to read content of cache file {}: {:?}",
            cache_file.display(),
            e
        )
    })?;
    if contents.is_empty() {
        return Ok(Value::Null);
    }
    Value::from_str(&contents).map_err(|e| {
        anyhow!(
            "invalid json content for cache file {}: {:?}",
            cache_file.display(),
            e
        )
    })
}

pub(super) async fn load_ipv4_from_cache(
    config: &DirectFloatEscaperConfig,
) -> anyhow::Result<BindSet> {
    if let Some(cache_file) = &config.cache_ipv4 {
        let records = load_records_from_cache(cache_file).await?;
        BindSet::parse_json(&records, AddressFamily::Ipv4)
    } else {
        Ok(BindSet::new(AddressFamily::Ipv4))
    }
}

pub(super) async fn load_ipv6_from_cache(
    config: &DirectFloatEscaperConfig,
) -> anyhow::Result<BindSet> {
    if let Some(cache_file) = &config.cache_ipv6 {
        let records = load_records_from_cache(cache_file).await?;
        BindSet::parse_json(&records, AddressFamily::Ipv6)
    } else {
        Ok(BindSet::new(AddressFamily::Ipv6))
    }
}

async fn parse_value(
    value: Value,
    family: AddressFamily,
    cache_file: &Option<PathBuf>,
) -> anyhow::Result<BindSet> {
    let binds = BindSet::parse_json(&value, family)?;

    if let Some(cache_file) = cache_file {
        let content = serde_json::to_string_pretty(&value).map_err(|e| {
            anyhow!(
                "failed to encoding {} records as json string: {:?}",
                family,
                e
            )
        })?;
        if let Some(executed) =
            crate::control::run_protected_io(tokio::fs::write(cache_file, content)).await
        {
            executed.map_err(|e| {
                anyhow!(
                    "failed to write to cache file {}: {:?}",
                    cache_file.display(),
                    e
                )
            })?
        }
    }

    Ok(binds)
}

pub(super) async fn publish_records(
    config: &DirectFloatEscaperConfig,
    v4_container: &ArcSwap<BindSet>,
    v6_container: &ArcSwap<BindSet>,
    data: String,
) -> anyhow::Result<()> {
    let obj =
        Value::from_str(&data).map_err(|e| anyhow!("the input data is not valid json: {:?}", e))?;

    if let Value::Object(map) = obj {
        for (k, v) in map.into_iter() {
            match g3_json::key::normalize(&k).as_str() {
                "ipv4" | "v4" => {
                    let bind_set = parse_value(v, AddressFamily::Ipv4, &config.cache_ipv4).await?;
                    v4_container.store(Arc::new(bind_set));
                }
                "ipv6" | "v6" => {
                    let bind_set = parse_value(v, AddressFamily::Ipv6, &config.cache_ipv6).await?;
                    v6_container.store(Arc::new(bind_set));
                }
                _ => return Err(anyhow!("no action defined for key {}", k)),
            }
        }
        Ok(())
    } else {
        Err(anyhow!("the input data should be json map"))
    }
}
