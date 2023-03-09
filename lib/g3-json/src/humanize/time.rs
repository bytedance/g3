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
use std::time::Duration;

use anyhow::anyhow;
use humanize_rs::ParseError;
use serde_json::Value;

pub fn as_duration(v: &Value) -> anyhow::Result<Duration> {
    match v {
        Value::String(value) => match humanize_rs::duration::parse(value) {
            Ok(v) => Ok(v),
            Err(ParseError::MissingUnit) => {
                if let Ok(u) = u64::from_str(value) {
                    Ok(Duration::from_secs(u))
                } else if let Ok(f) = f64::from_str(value) {
                    Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
                } else {
                    Err(anyhow!("unsupported duration string"))
                }
            }
            Err(e) => Err(anyhow!("invalid humanize duration string: {e}")),
        },
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(Duration::from_secs(u))
            } else if let Some(f) = n.as_f64() {
                Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
            } else {
                Err(anyhow!("unsupported duration string"))
            }
        }
        _ => Err(anyhow!(
            "json value type for humanize duration should be 'string' or 'integer' or 'real'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn t_duration() {
        let j = json!({"v": "1h2m"});
        assert_eq!(
            as_duration(&j["v"]).unwrap(),
            Duration::from_secs(3600 + 120)
        );

        let j = json!({"v": "1000"});
        assert_eq!(as_duration(&j["v"]).unwrap(), Duration::from_secs(1000));

        let j = json!({"v": "-1000"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "1.01"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "-1000h"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "1000Ah"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": 1000});
        assert_eq!(as_duration(&j["v"]).unwrap(), Duration::from_secs(1000));

        let j = json!({"v": -1000});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": 1.01});
        assert_eq!(
            as_duration(&j["v"]).unwrap(),
            Duration::try_from_secs_f64(1.01).unwrap()
        );

        let j = json!({"v": ["1"]});
        assert!(as_duration(&j["v"]).is_err());
    }
}
