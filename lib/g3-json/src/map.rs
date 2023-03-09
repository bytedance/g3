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

use anyhow::anyhow;
use serde_json::{Map, Value};

pub fn get_required_str<'a>(map: &'a Map<String, Value>, k: &str) -> anyhow::Result<&'a str> {
    match map.get(k) {
        Some(v) => match v {
            Value::String(s) => Ok(s),
            _ => Err(anyhow!("invalid string value for key {k}")),
        },
        None => Err(anyhow!("no key {k} found in this map")),
    }
}

pub fn get_required<'a>(map: &'a Map<String, Value>, k: &str) -> anyhow::Result<&'a Value> {
    match map.get(k) {
        Some(v) => Ok(v),
        None => Err(anyhow!("no key {k} found in this map")),
    }
}
