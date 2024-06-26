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
use arc_swap::ArcSwap;
use futures_util::future::{AbortHandle, Abortable};
use log::warn;

use super::PeerSet;
use crate::config::escaper::proxy_float::{ProxyFloatEscaperConfig, ProxyFloatSource};

mod file;
mod redis;
mod redis_cluster;

pub(super) async fn load_cached_peers(
    config: &Arc<ProxyFloatEscaperConfig>,
) -> anyhow::Result<PeerSet> {
    if let Some(cache_file) = &config.cache_file {
        let records = file::load_peers_from_cache(cache_file).await?;
        super::peer::parse_peers(config, &records)
    } else {
        Ok(PeerSet::default())
    }
}

async fn parse_and_save_peers(
    config: &Arc<ProxyFloatEscaperConfig>,
    container: &Arc<ArcSwap<PeerSet>>,
    records: Vec<serde_json::Value>,
) -> anyhow::Result<()> {
    let peers = super::peer::parse_peers(config, &records)
        .map_err(|e| anyhow!("failed to parse peers: {e:?}"))?;

    container.store(Arc::new(peers));
    if let Some(cache_file) = &config.cache_file {
        file::save_peers_to_cache(cache_file, records)
            .await
            .map_err(|e| anyhow!("failed to cache peers: {e:?}"))?;
    }
    Ok(())
}

pub(super) async fn publish_peers(
    config: &Arc<ProxyFloatEscaperConfig>,
    peers_container: &Arc<ArcSwap<PeerSet>>,
    data: String,
) -> anyhow::Result<()> {
    let obj = serde_json::from_str(&data)
        .map_err(|e| anyhow!("the publish data is not valid json: {e:?}"))?;
    let records = match obj {
        serde_json::Value::Array(v) => v,
        serde_json::Value::Object(_) => vec![obj],
        _ => return Err(anyhow!("invalid input json data type")),
    };

    parse_and_save_peers(config, peers_container, records).await
}

pub(super) fn new_job(
    config: Arc<ProxyFloatEscaperConfig>,
    peers_container: Arc<ArcSwap<PeerSet>>,
) -> anyhow::Result<AbortHandle> {
    let f = async move {
        let mut interval = tokio::time::interval(config.refresh_interval);
        interval.tick().await; // will tick immediately
        loop {
            let result = match &config.source {
                ProxyFloatSource::Passive => {
                    // do nothing
                    interval.tick().await;
                    continue;
                }
                ProxyFloatSource::Redis(config) => redis::fetch_records(config).await,
                ProxyFloatSource::RedisCluster(config) => {
                    redis_cluster::fetch_records(config).await
                }
            };
            match result {
                Ok(records) => {
                    if let Err(e) = parse_and_save_peers(&config, &peers_container, records).await {
                        warn!("failed to update peers for escaper {}: {e:?}", config.name);
                    }
                }
                Err(e) => warn!("failed to fetch peers for escaper {}: {e:?}", config.name),
            }

            interval.tick().await;
        }
    };

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let future = Abortable::new(f, abort_registration);
    tokio::spawn(future);
    Ok(abort_handle)
}
