/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use governor::Quota;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitQuotaConfig(Quota);

impl RateLimitQuotaConfig {
    pub const fn per_second(count: NonZeroU32) -> Self {
        RateLimitQuotaConfig(Quota::per_second(count))
    }

    pub fn with_period(replenish_1_per: Duration) -> Option<Self> {
        Quota::with_period(replenish_1_per).map(RateLimitQuotaConfig)
    }

    pub fn allow_burst(&mut self, max_burst: NonZeroU32) {
        self.0 = self.0.allow_burst(max_burst);
    }

    pub fn get_inner(&self) -> Quota {
        self.0
    }
}

impl FromStr for RateLimitQuotaConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('/') {
            Some((v1, v2)) => {
                let u = NonZeroU32::from_str(v1.trim())
                    .map_err(|_| anyhow!("invalid non-zero u32 string as the first part"))?;
                match v2 {
                    "s" => Ok(RateLimitQuotaConfig(Quota::per_second(u))),
                    "m" => Ok(RateLimitQuotaConfig(Quota::per_minute(u))),
                    "h" => Ok(RateLimitQuotaConfig(Quota::per_hour(u))),
                    _ => Err(anyhow!("invalid unit in second part")),
                }
            }
            None => {
                let u = NonZeroU32::from_str(s)
                    .map_err(|e| anyhow!("invalid non-zero u32 string: {e}"))?;
                Ok(RateLimitQuotaConfig(Quota::per_second(u)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_from_str() {
        assert_eq!(
            RateLimitQuotaConfig::from_str("30").unwrap(),
            RateLimitQuotaConfig::per_second(NonZeroU32::new(30).unwrap())
        );
        assert_eq!(
            RateLimitQuotaConfig::from_str("30/s").unwrap(),
            RateLimitQuotaConfig::per_second(NonZeroU32::new(30).unwrap())
        );

        let mut v = RateLimitQuotaConfig::with_period(Duration::from_secs(1)).unwrap();
        v.allow_burst(NonZeroU32::new(60).unwrap());
        assert_eq!(RateLimitQuotaConfig::from_str("60/m").unwrap(), v);

        v.allow_burst(NonZeroU32::new(3600).unwrap());
        assert_eq!(RateLimitQuotaConfig::from_str("3600/h").unwrap(), v);
    }
}
