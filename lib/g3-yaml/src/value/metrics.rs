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

use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;
use g3_types::metrics::{MetricsName, MetricsTagName, MetricsTagValue, StaticMetricsTags};

pub fn as_metrics_name(v: &Yaml) -> anyhow::Result<MetricsName> {
    if let Yaml::String(s) = v {
        let name = MetricsName::from_str(s).map_err(|e| anyhow!("invalid metrics name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "yaml value type for 'metrics name' should be 'string'"
        ))
    }
}

pub fn as_static_metrics_tags(v: &Yaml) -> anyhow::Result<StaticMetricsTags> {
    if let Yaml::Hash(map) = v {
        let mut tags = BTreeMap::new();
        crate::foreach_kv(map, |k, v| {
            let name = MetricsTagName::from_str(k).context("invalid metrics tag name")?;
            let value_s = crate::value::as_string(v).context("invalid metrics tag yaml value")?;
            let value = MetricsTagValue::from_str(&value_s).context("invalid metrics tag value")?;

            if tags.insert(name, value).is_some() {
                Err(anyhow!("found duplicate value for tag name {k}"))
            } else {
                Ok(())
            }
        })?;
        Ok(tags)
    } else {
        Err(anyhow!(
            "the yaml value type for 'static metric tags' should be 'map'"
        ))
    }
}

pub fn as_weighted_metrics_name(value: &Yaml) -> anyhow::Result<WeightedValue<MetricsName>> {
    if let Yaml::Hash(map) = value {
        let mut name = MetricsName::default();
        let mut weight = None;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "name" => {
                name = as_metrics_name(v)?;
                Ok(())
            }
            "weight" => {
                let f = crate::value::as_f64(v)?;
                weight = Some(f);
                Ok(())
            }
            _ => Ok(()),
        })?;

        if name.is_empty() {
            Err(anyhow!("no name found"))
        } else if let Some(weight) = weight {
            Ok(WeightedValue::with_weight(name, weight))
        } else {
            Ok(WeightedValue::new(name))
        }
    } else {
        let name = as_metrics_name(value)?;
        Ok(WeightedValue::new(name))
    }
}
