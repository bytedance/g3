/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use yaml_rust::Yaml;

pub fn as_rfc3339_datetime(value: &Yaml) -> anyhow::Result<DateTime<Utc>> {
    match value {
        Yaml::String(s) => {
            let datetime = DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow!("invalid rfc3339 datetime string: {e}"))?;
            Ok(datetime.with_timezone(&Utc))
        }
        _ => Err(anyhow!(
            "yaml value type for 'rfc3339 datetime' should be string"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_rfc3339_datetime_ok() {
        let value = yaml_str!("2019-05-23T17:38:00Z");
        assert_eq!(
            as_rfc3339_datetime(&value).unwrap().to_rfc3339(),
            "2019-05-23T17:38:00+00:00"
        );

        let value = yaml_str!("2020-06-02T12:00:00+08:00");
        assert_eq!(
            as_rfc3339_datetime(&value).unwrap().to_rfc3339(),
            "2020-06-02T04:00:00+00:00"
        );

        let value = yaml_str!("2023-01-01T12:00:00-05:00");
        assert_eq!(
            as_rfc3339_datetime(&value).unwrap().to_rfc3339(),
            "2023-01-01T17:00:00+00:00"
        );

        let value = yaml_str!("2025-11-12T12:00:00.123Z");
        assert_eq!(
            as_rfc3339_datetime(&value).unwrap().to_rfc3339(),
            "2025-11-12T12:00:00.123+00:00"
        );

        let value = yaml_str!("2016-12-31T23:59:60Z");
        assert_eq!(
            as_rfc3339_datetime(&value).unwrap().to_rfc3339(),
            "2016-12-31T23:59:60+00:00"
        );
    }

    #[test]
    fn as_rfc3339_datetime_err() {
        let value = yaml_str!("2022-01-01T12:00:00");
        assert!(as_rfc3339_datetime(&value).is_err());

        let value = yaml_str!("2023-02-30T00:00:00Z");
        assert!(as_rfc3339_datetime(&value).is_err());

        let value = yaml_str!("2024-03-01T25:00:00Z");
        assert!(as_rfc3339_datetime(&value).is_err());

        let value = Yaml::Integer(12345);
        assert!(as_rfc3339_datetime(&value).is_err());

        let value = Yaml::Boolean(true);
        assert!(as_rfc3339_datetime(&value).is_err());
    }
}
