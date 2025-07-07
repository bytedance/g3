/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::limit::RateLimitQuotaConfig;

pub fn as_rate_limit_quota(v: &Yaml) -> anyhow::Result<RateLimitQuotaConfig> {
    match v {
        Yaml::Integer(_) => {
            let count = crate::value::as_nonzero_u32(v)?;
            Ok(RateLimitQuotaConfig::per_second(count))
        }
        Yaml::String(s) => RateLimitQuotaConfig::from_str(s),
        Yaml::Hash(map) => {
            let mut quota: Option<RateLimitQuotaConfig> = None;
            let mut max_burst: Option<NonZeroU32> = None;
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "rate" => match v {
                    Yaml::Integer(_) | Yaml::String(_) => {
                        quota = Some(
                            as_rate_limit_quota(v).context(format!("invalid value for key {k}"))?,
                        );
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid value type for key {k}")),
                },
                "replenish_interval" => {
                    let dur = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    quota = RateLimitQuotaConfig::with_period(dur);
                    Ok(())
                }
                "max_burst" => {
                    max_burst = Some(
                        crate::value::as_nonzero_u32(v)
                            .context(format!("invalid nonzero u32 value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            match quota {
                Some(mut quota) => {
                    if let Some(max_burst) = max_burst {
                        quota.allow_burst(max_burst);
                    }
                    Ok(quota)
                }
                None => Err(anyhow!("no rate / replenish_interval is set")),
            }
        }
        _ => Err(anyhow!("invalid yaml value type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_rate_limit_quota_ok() {
        let ten = NonZeroU32::new(10).unwrap();
        let exp = RateLimitQuotaConfig::per_second(ten);

        let v = Yaml::Integer(10);
        let quota = as_rate_limit_quota(&v).unwrap();
        assert_eq!(quota, exp);

        let v = yaml_str!("10");
        let quota = as_rate_limit_quota(&v).unwrap();
        assert_eq!(quota, exp);

        let v = yaml_str!("10/s");
        let quota = as_rate_limit_quota(&v).unwrap();
        assert_eq!(quota, exp);

        let ten = NonZeroU32::new(10).unwrap();
        let thirty = NonZeroU32::new(30).unwrap();
        let mut exp = RateLimitQuotaConfig::per_second(ten);
        exp.allow_burst(thirty);

        let yaml = yaml_doc!(
            "
            rate: 10
            max_burst: 30
            "
        );
        let quota = as_rate_limit_quota(&yaml).unwrap();
        assert_eq!(quota, exp);

        let yaml = yaml_doc!(
            "
            rate: 10/s
            max_burst: 30
            "
        );
        let quota = as_rate_limit_quota(&yaml).unwrap();
        assert_eq!(quota, exp);

        let yaml = yaml_doc!(
            "
            replenish_interval: 100ms
            max_burst: 30
            "
        );
        let quota = as_rate_limit_quota(&yaml).unwrap();
        assert_eq!(quota, exp);
    }

    #[test]
    fn as_rate_limit_quota_err() {
        // invalid value type for key rate
        let yaml = yaml_doc!("rate: []");
        assert!(as_rate_limit_quota(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!("invalid: 10");
        assert!(as_rate_limit_quota(&yaml).is_err());

        // no rate / replenish_interval is set
        let yaml = yaml_doc!("max_burst: 30");
        assert!(as_rate_limit_quota(&yaml).is_err());

        // invalid yaml value type
        let yaml = Yaml::Null;
        assert!(as_rate_limit_quota(&yaml).is_err());
    }
}
