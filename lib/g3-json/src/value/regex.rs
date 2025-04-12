/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use regex::Regex;
use serde_json::Value;

pub fn as_regex(value: &Value) -> anyhow::Result<Regex> {
    if let Value::String(s) = value {
        let regex = Regex::new(s).map_err(|e| anyhow!("invalid regex value: {e}"))?;
        Ok(regex)
    } else {
        Err(anyhow!(
            "the yaml value type for regex string should be 'string'"
        ))
    }
}
