/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::num::{NonZeroU32, NonZeroU64};
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;

use g3_std_ext::time::DurationExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateLimitQuota {
    pub(super) max_burst: NonZeroU32,
    pub(super) replenish_nanos: NonZeroU64,
}

impl RateLimitQuota {
    pub fn new(period: Duration, max_burst: NonZeroU32) -> anyhow::Result<Self> {
        let replenish_nanos = period.as_nanos_u64() / (max_burst.get() as u64);
        let replenish_nanos = NonZeroU64::new(replenish_nanos).ok_or_else(|| {
            anyhow!("too large max burst value {max_burst} within {period:?} period")
        })?;
        Ok(RateLimitQuota {
            max_burst,
            replenish_nanos,
        })
    }

    pub fn per_second(max_burst: NonZeroU32) -> anyhow::Result<Self> {
        Self::new(Duration::from_secs(1), max_burst)
    }

    pub fn per_minute(max_burst: NonZeroU32) -> anyhow::Result<Self> {
        Self::new(Duration::from_secs(60), max_burst)
    }

    pub fn per_hour(max_burst: NonZeroU32) -> anyhow::Result<Self> {
        Self::new(Duration::from_secs(3600), max_burst)
    }

    pub fn with_period(period: Duration) -> Option<Self> {
        let replenish_nanos = NonZeroU64::new(period.as_nanos_u64())?;
        Some(RateLimitQuota {
            max_burst: NonZeroU32::MIN,
            replenish_nanos,
        })
    }

    pub fn allow_burst(&mut self, max_burst: NonZeroU32) {
        self.max_burst = max_burst;
    }
}

impl FromStr for RateLimitQuota {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('/') {
            Some((v1, v2)) => {
                let u = NonZeroU32::from_str(v1.trim())
                    .map_err(|_| anyhow!("invalid non-zero u32 string as the first part"))?;
                match v2 {
                    "s" => RateLimitQuota::per_second(u),
                    "m" => RateLimitQuota::per_minute(u),
                    "h" => RateLimitQuota::per_hour(u),
                    _ => Err(anyhow!("invalid unit in second part")),
                }
            }
            None => {
                let u = NonZeroU32::from_str(s)
                    .map_err(|e| anyhow!("invalid non-zero u32 string: {e}"))?;
                RateLimitQuota::per_second(u)
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
            RateLimitQuota::from_str("30").unwrap(),
            RateLimitQuota::per_second(NonZeroU32::new(30).unwrap()).unwrap()
        );
        assert_eq!(
            RateLimitQuota::from_str("30/s").unwrap(),
            RateLimitQuota::per_second(NonZeroU32::new(30).unwrap()).unwrap()
        );

        let mut v = RateLimitQuota::with_period(Duration::from_secs(1)).unwrap();
        v.allow_burst(NonZeroU32::new(60).unwrap());
        assert_eq!(RateLimitQuota::from_str("60/m").unwrap(), v);

        v.allow_burst(NonZeroU32::new(3600).unwrap());
        assert_eq!(RateLimitQuota::from_str("3600/h").unwrap(), v);
    }
}
