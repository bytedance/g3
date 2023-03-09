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
use redis::{Commands, ConnectionAddr, ConnectionInfo, RedisConnectionInfo};

use crate::config::escaper::proxy_float::source::redis_cluster::ProxyFloatRedisClusterSource;

pub(super) async fn fetch_records(
    source: &Arc<ProxyFloatRedisClusterSource>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    // the async in redis crate is not ready yet
    let source = Arc::clone(source);
    tokio::task::spawn_blocking(move || fetch_records_blocking(source))
        .await
        .map_err(|e| anyhow!("join blocking task error: {e:?}"))?
}

pub(super) fn fetch_records_blocking(
    source: Arc<ProxyFloatRedisClusterSource>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut initial_nodes = Vec::<ConnectionInfo>::new();
    for node in &source.initial_nodes {
        initial_nodes.push(ConnectionInfo {
            addr: ConnectionAddr::Tcp(node.host().to_string(), node.port()),
            redis: RedisConnectionInfo {
                db: 0,
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
    let client = client_builder.build()?;
    let mut con = client
        .get_connection()
        .map_err(|e| anyhow!("connect failed: {e:?}"))?;
    con.set_read_timeout(Some(source.read_timeout))
        .map_err(|e| {
            anyhow!(
                "unable set read timeout to {:?}: {e:?}",
                source.read_timeout
            )
        })?;
    let members: Vec<String> = con.smembers(&source.sets_key).map_err(|e| {
        anyhow!(
            "failed to get all members for sets {}: {e:?}",
            source.sets_key
        )
    })?;
    let mut records = Vec::<serde_json::Value>::new();
    for member in &members {
        let record =
            serde_json::from_str(member).map_err(|e| anyhow!("found invalid member: {e:?}"))?;
        records.push(record);
    }
    Ok(records)
}
