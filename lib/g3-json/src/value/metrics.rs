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

use anyhow::anyhow;
use serde_json::Value;

use g3_types::collection::WeightedValue;
use g3_types::metrics::MetricsName;

pub fn as_metrics_name(v: &Value) -> anyhow::Result<MetricsName> {
    if let Value::String(s) = v {
        let name = MetricsName::from_str(s).map_err(|e| anyhow!("invalid metrics name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "json value type for 'metrics name' should be string"
        ))
    }
}

pub fn as_weighted_metrics_name(value: &Value) -> anyhow::Result<WeightedValue<MetricsName>> {
    if let Value::Object(map) = value {
        let mut name = MetricsName::default();
        let mut weight = None;

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "name" => name = as_metrics_name(v)?,
                "weight" => {
                    let f = crate::value::as_f64(v)?;
                    weight = Some(f);
                }
                _ => {}
            }
        }

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
