/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::Context;
use chrono::{DateTime, Utc};

use std::time::Duration;

use g3_types::net::OpensslTicketKey;

mod json;
#[cfg(feature = "yaml")]
mod yaml;

mod redis;
use redis::{RedisSource, RedisSourceConfig};

const CONFIG_KEY_SOURCE_TYPE: &str = "type";

pub(crate) struct RemoteEncryptKey {
    pub(crate) key: OpensslTicketKey,
}

pub(crate) struct RemoteDecryptKey {
    pub(crate) key: OpensslTicketKey,
    expire: DateTime<Utc>,
}

impl RemoteDecryptKey {
    pub(crate) fn expire_duration(&self, now: &DateTime<Utc>) -> Option<Duration> {
        self.expire.signed_duration_since(now).to_std().ok()
    }
}

pub(crate) struct RemoteKeys {
    pub(crate) enc: RemoteEncryptKey,
    pub(crate) dec: Vec<RemoteDecryptKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TicketSourceConfig {
    Redis(RedisSourceConfig),
}

impl TicketSourceConfig {
    pub(crate) fn build(&self) -> anyhow::Result<TicketSource> {
        match self {
            TicketSourceConfig::Redis(s) => {
                let source = s
                    .build()
                    .context("failed to build redis remote key source")?;
                Ok(TicketSource::Redis(source))
            }
        }
    }
}

pub(crate) enum TicketSource {
    Redis(RedisSource),
}

impl TicketSource {
    pub(crate) async fn fetch_remote_keys(&self) -> anyhow::Result<RemoteKeys> {
        match self {
            TicketSource::Redis(s) => s
                .fetch_remote_keys()
                .await
                .context("failed to fetch remote keys from redis"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use g3_types::net::OpensslTicketKeyBuilder;

    // Helper function to create a test RemoteDecryptKey
    fn create_test_decrypt_key(expire: DateTime<Utc>) -> RemoteDecryptKey {
        let builder = OpensslTicketKeyBuilder::default();
        RemoteDecryptKey {
            key: builder.build(),
            expire,
        }
    }

    #[test]
    fn remote_decrypt_key_expire_duration_future() {
        let future_time = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();

        let key = create_test_decrypt_key(future_time);
        let duration = key.expire_duration(&now);

        assert!(duration.is_some());
        let dur = duration.unwrap();
        assert_eq!(dur.as_secs(), (future_time - now).num_seconds() as u64);
    }

    #[test]
    fn remote_decrypt_key_expire_duration_past() {
        let past_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();

        let key = create_test_decrypt_key(past_time);
        let duration = key.expire_duration(&now);

        assert!(duration.is_none());
    }
}
