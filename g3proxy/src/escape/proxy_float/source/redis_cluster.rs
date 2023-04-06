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
use redis::{AsyncCommands, ConnectionAddr, ConnectionInfo, RedisConnectionInfo};

use crate::config::escaper::proxy_float::source::redis_cluster::ProxyFloatRedisClusterSource;

async fn connect_to_redis_cluster(
    source: &Arc<ProxyFloatRedisClusterSource>,
) -> anyhow::Result<impl AsyncCommands> {
    let mut initial_nodes = Vec::<ConnectionInfo>::new();
    for node in &source.initial_nodes {
        initial_nodes.push(ConnectionInfo {
            addr: ConnectionAddr::Tcp(node.host().to_string(), node.port()),
            redis: RedisConnectionInfo {
                db: 0, // database is always 0 according to https://redis.io/docs/reference/cluster-spec/
                username: None,
                password: None,
            },
        })
    }
    let mut client_builder =
        redis::cluster::ClusterClientBuilder::new(initial_nodes).read_from_replicas();
    if let Some(username) = &source.username {
        client_builder = client_builder.username(username.to_string());
    }
    if let Some(password) = &source.password {
        client_builder = client_builder.password(password.to_string());
    }
    let client = client_builder
        .build()
        .map_err(|e| anyhow!("failed to build redis cluster client: {e}"))?;

    match tokio::time::timeout(source.connect_timeout, client.get_async_connection()).await {
        Ok(Ok(con)) => Ok(con),
        Ok(Err(e)) => Err(anyhow!("connect failed: {e}")),
        Err(_) => Err(anyhow!("connect timeout")),
    }
}

pub(super) async fn fetch_records(
    source: &Arc<ProxyFloatRedisClusterSource>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let con = connect_to_redis_cluster(source).await?;
    super::redis::get_members(con, source.read_timeout, &source.sets_key).await
}
