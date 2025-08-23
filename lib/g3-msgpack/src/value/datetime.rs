/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use rmpv::ValueRef;

pub fn as_rfc3339_datetime(value: &ValueRef) -> anyhow::Result<DateTime<Utc>> {
    match value {
        ValueRef::String(s) => match s.as_str() {
            Some(s) => {
                let datetime = DateTime::parse_from_rfc3339(s)
                    .map_err(|e| anyhow!("invalid rfc3339 datetime string: {e}"))?;
                Ok(datetime.with_timezone(&Utc))
            }
            None => Err(anyhow!("invalid utf-8 string")),
        },
        _ => Err(anyhow!(
            "yaml value type for 'rfc3339 datetime' should be string"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Utf8StringRef;

    #[test]
    fn as_rfc3339_datetime_ok() {
        let value = ValueRef::String(Utf8StringRef::from("2019-05-23T17:38:00Z"));
        let dt = as_rfc3339_datetime(&value).unwrap();
        assert_eq!(dt.to_rfc3339(), "2019-05-23T17:38:00+00:00");
    }

    #[test]
    fn as_rfc3339_datetime_err() {
        let value = ValueRef::String(Utf8StringRef::from("2019-05-23 17:38:00"));
        assert!(as_rfc3339_datetime(&value).is_err());

        let value = ValueRef::F32(1.0);
        assert!(as_rfc3339_datetime(&value).is_err());
    }
}
