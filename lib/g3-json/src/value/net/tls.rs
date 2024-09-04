/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Value) -> anyhow::Result<TlsVersion> {
    match value {
        Value::String(s) => TlsVersion::from_str(s),
        Value::Number(n) => {
            let Some(f) = n.as_f64() else {
                return Err(anyhow!("invalid f64 number value"));
            };
            TlsVersion::try_from(f)
        }
        _ => Err(anyhow!(
            "json value type for tls version should be 'string' or 'float'"
        )),
    }
}
