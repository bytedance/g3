/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
