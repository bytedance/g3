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

use std::convert::TryFrom;

use anyhow::anyhow;
use humanize_rs::bytes::Bytes;
use serde_json::Value;

pub fn as_usize(v: &Value) -> anyhow::Result<usize> {
    match v {
        Value::String(s) => {
            let v = s.parse::<Bytes>()?;
            Ok(v.size())
        }
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(usize::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for usize"))
            }
        }
        _ => Err(anyhow!(
            "yaml value type for humanize usize should be 'string' or 'integer'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn t_usize() {
        let j = json!({"v": "1000"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1K"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KB"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KiB"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1024);

        let j = json!({"v": 1024});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1024);

        let j = json!({"v": -1024});
        assert!(as_usize(&j["v"]).is_err());

        let j = json!({"v": 1.01});
        assert!(as_usize(&j["v"]).is_err());

        let j = json!({"v": ["1"]});
        assert!(as_usize(&j["v"]).is_err());
    }
}
