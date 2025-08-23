/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use redis::AsyncCommands;
use serde_json::Value;

use g3_redis_client::RedisClientConfig;

use super::FetchJob;
use crate::config::escaper::proxy_float::source::redis::ProxyFloatRedisSource;

pub(super) struct RedisFetchJob {
    client: RedisClientConfig,
    sets_key: String,
}

impl RedisFetchJob {
    pub(super) fn new(config: &ProxyFloatRedisSource) -> anyhow::Result<Self> {
        let client = config.client_builder.build()?;
        Ok(RedisFetchJob {
            client,
            sets_key: config.sets_key.clone(),
        })
    }
}

impl FetchJob for RedisFetchJob {
    async fn fetch_records(&self) -> anyhow::Result<Vec<Value>> {
        let mut con = self.client.connect().await?;

        let members: Vec<redis::Value> = con
            .smembers(&self.sets_key)
            .await
            .map_err(|e| anyhow!("failed to get all members of sets {}: {e}", self.sets_key))?;

        let mut records = Vec::<serde_json::Value>::new();
        for member in &members {
            let redis::Value::BulkString(b) = member else {
                return Err(anyhow!("invalid member data type in set {}", self.sets_key));
            };
            let record = serde_json::from_slice(b)
                .map_err(|e| anyhow!("invalid member in set {}: {e}", self.sets_key))?;
            records.push(record);
        }
        Ok(records)
    }
}
