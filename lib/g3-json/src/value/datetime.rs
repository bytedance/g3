/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

    #[test]
    fn utc_tz() {
        let value = Value::String("2019-05-23T17:38:00Z".to_string());
        let dt = as_rfc3339_datetime(&value).unwrap();
        assert_eq!(dt.to_rfc3339(), "2019-05-23T17:38:00+00:00");
    }
}
