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
use std::time::Duration;

use anyhow::anyhow;
use redis::AsyncCommands;

use crate::config::escaper::proxy_float::source::redis::ProxyFloatRedisSource;

async fn connect_to_redis(
    source: &Arc<ProxyFloatRedisSource>,
) -> anyhow::Result<impl AsyncCommands> {
    let client = redis::Client::open(source.as_ref())
        .map_err(|e| anyhow!("redis client open failed: {e}"))?;
    match tokio::time::timeout(source.connect_timeout, client.get_async_connection()).await {
        Ok(Ok(con)) => Ok(con),
        Ok(Err(e)) => Err(anyhow!("connect failed: {e:}")),
        Err(_) => Err(anyhow!("connect timeout")),
    }
}

pub(super) async fn get_members<C: AsyncCommands>(
    mut con: C,
    timeout: Duration,
    sets_key: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let members: Vec<String> = match tokio::time::timeout(timeout, con.smembers(sets_key)).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            return Err(anyhow!(
                "failed to get all members for sets {sets_key}: {e}"
            ))
        }
        Err(_) => return Err(anyhow!("timeout to get all members for sets {sets_key}",)),
    };

    let mut records = Vec::<serde_json::Value>::new();
    for member in &members {
        let record =
            serde_json::from_str(member).map_err(|e| anyhow!("found invalid member: {e}"))?;
        records.push(record);
    }
    Ok(records)
}

pub(super) async fn fetch_records(
    source: &Arc<ProxyFloatRedisSource>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let con = connect_to_redis(source).await?;
    get_members(con, source.read_timeout, &source.sets_key).await
}
