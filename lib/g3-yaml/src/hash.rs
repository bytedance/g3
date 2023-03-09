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
use yaml_rust::{yaml, Yaml};

pub fn foreach_kv<F>(table: &yaml::Hash, mut f: F) -> anyhow::Result<()>
where
    F: FnMut(&str, &Yaml) -> anyhow::Result<()>,
{
    for (k, v) in table.iter() {
        if let Yaml::String(key) = k {
            f(key, v).context(format!("failed to parse value of key {key}"))?;
        } else {
            return Err(anyhow!("key in hash should be string"));
        }
    }
    Ok(())
}

pub fn get_required<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a Yaml> {
    let key = Yaml::String(k.to_owned());
    match map.get(&key) {
        Some(v) => Ok(v),
        None => Err(anyhow!("no required key {k} found in this map")),
    }
}

pub fn get_required_str<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a str> {
    let v = get_required(map, k)?;
    if let Yaml::String(s) = v {
        Ok(s)
    } else {
        Err(anyhow!("invalid string value for required key {k}"))
    }
}
