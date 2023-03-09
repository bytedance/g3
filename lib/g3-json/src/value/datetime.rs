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
use chrono::{DateTime, Utc};
use serde_json::Value;

pub fn as_rfc3339_datetime(v: &Value) -> anyhow::Result<DateTime<Utc>> {
    match v {
        Value::String(s) => {
            let datetime = DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow!("invalid rfc3339 datetime string: {e}"))?;
            Ok(datetime.with_timezone(&Utc))
        }
        _ => Err(anyhow!(
            "json value type for 'rfc3339 datetime' should be string"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn utc_tz() {
        let value = Value::String("2019-05-23T17:38:00Z".to_string());
        let dt = as_rfc3339_datetime(&value).unwrap();
        assert_eq!(dt.to_rfc3339(), "2019-05-23T17:38:00+00:00");
    }
}
