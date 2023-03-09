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
use rand::distributions::Bernoulli;
use serde_json::Value;

pub fn as_random_ratio(value: &Value) -> anyhow::Result<Bernoulli> {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
            } else {
                Err(anyhow!("invalid f64 ration value"))
            }
        }
        Value::String(s) => {
            if let Some(p) = s.find('/') {
                let n1 = u32::from_str(s[0..p].trim())
                    .map_err(|e| anyhow!("first part is not valid u32: {e}"))?;
                let n2 = u32::from_str(s[p + 1..].trim())
                    .map_err(|e| anyhow!("second part is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n1, n2)
                    .map_err(|e| anyhow!("out of range fraction ratio: {e}"))
            } else if let Some(s) = s.strip_suffix('%') {
                let n = u32::from_str(s.trim())
                    .map_err(|e| anyhow!("the part before % is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n, 100)
                    .map_err(|e| anyhow!("out of range percentage ratio: {e}"))
            } else {
                let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 ratio string: {e}"))?;
                Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
            }
        }
        Value::Bool(true) => Ok(Bernoulli::new(1.0).unwrap()),
        Value::Bool(false) => Ok(Bernoulli::new(0.0).unwrap()),
        _ => Err(anyhow!(
            "yaml value type for 'random ratio' should be 'f64' or 'string'"
        )),
    }
}
