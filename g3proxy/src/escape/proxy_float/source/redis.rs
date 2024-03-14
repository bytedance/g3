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

use std::sync::Arc;

use anyhow::anyhow;
use redis::AsyncCommands;

use crate::config::escaper::proxy_float::source::redis::ProxyFloatRedisSource;

async fn connect_to_redis(
    source: &Arc<ProxyFloatRedisSource>,
) -> anyhow::Result<impl AsyncCommands> {
    let client = redis::Client::open(source.as_ref())
        .map_err(|e| anyhow!("redis client open failed: {e}"))?;
    client
        .get_multiplexed_async_connection_with_timeouts(source.read_timeout, source.connect_timeout)
        .await
        .map_err(|e| anyhow!("connect to redis failed: {e}"))
}

pub(super) async fn get_members<C: AsyncCommands>(
    mut con: C,
    sets_key: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let members: Vec<redis::Value> = con
        .smembers(sets_key)
        .await
        .map_err(|e| anyhow!("failed to get all members of sets {sets_key}: {e}"))?;

    let mut records = Vec::<serde_json::Value>::new();
    for member in &members {
        let redis::Value::Data(b) = member else {
            return Err(anyhow!("invalid member data type in set {sets_key}"));
        };
        let record = serde_json::from_slice(b)
            .map_err(|e| anyhow!("invalid member in set {sets_key}: {e}"))?;
        records.push(record);
    }
    Ok(records)
}

pub(super) async fn fetch_records(
    source: &Arc<ProxyFloatRedisSource>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let con = connect_to_redis(source).await?;
    get_members(con, &source.sets_key).await
}
