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
use rmpv::ValueRef;

use g3_types::collection::WeightedValue;
use g3_types::metrics::MetricsName;

pub fn as_metrics_name(v: &ValueRef) -> anyhow::Result<MetricsName> {
    if let ValueRef::String(s) = v {
        let s = s.as_str().ok_or_else(|| anyhow!("invalid utf-8 string"))?;
        let name = MetricsName::from_str(s).map_err(|e| anyhow!("invalid metrics name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "msgpack value type for 'metrics name' should be 'string'"
        ))
    }
}

pub fn as_weighted_metrics_name(v: &ValueRef) -> anyhow::Result<WeightedValue<MetricsName>> {
    match v {
        ValueRef::Map(map) => {
            let mut name = MetricsName::default();
            let mut weight = None;

            for (k, v) in map {
                let key = as_metrics_name(k).context("all keys should be metrics name")?;
                match crate::key::normalize(key.as_str()).as_str() {
                    "name" => {
                        name = as_metrics_name(v)
                            .context(format!("invalid metrics name value for key {key}"))?;
                    }
                    "weight" => {
                        let f = crate::value::as_f64(v)
                            .context(format!("invalid f64 value for key {key}"))?;
                        weight = Some(f);
                    }
                    _ => {} // ignore all other keys
                }
            }

            if name.is_empty() {
                Err(anyhow!("no name found"))
            } else if let Some(weight) = weight {
                Ok(WeightedValue::with_weight(name, weight))
            } else {
                Ok(WeightedValue::new(name))
            }
        }
        _ => {
            let s = as_metrics_name(v).context("invalid string value")?;
            Ok(WeightedValue::new(s))
        }
    }
}
