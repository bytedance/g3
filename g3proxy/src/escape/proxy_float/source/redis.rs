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
