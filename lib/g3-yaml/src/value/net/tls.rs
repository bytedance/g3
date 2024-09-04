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
use yaml_rust::Yaml;

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Yaml) -> anyhow::Result<TlsVersion> {
    match value {
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 value: {e}"))?;
            TlsVersion::try_from(f)
        }
        Yaml::String(s) => TlsVersion::from_str(s),
        _ => Err(anyhow!(
            "yaml value type for tls version should be 'string' or 'float'"
        )),
    }
}
