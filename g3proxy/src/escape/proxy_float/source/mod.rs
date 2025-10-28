/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use log::warn;
use tokio::sync::mpsc;

use super::PeerSet;
use crate::config::escaper::proxy_float::{ProxyFloatEscaperConfig, ProxyFloatSource};

mod file;
mod redis;

trait FetchJob {
    fn fetch_records(&self) -> impl Future<Output = anyhow::Result<Vec<serde_json::Value>>> + Send;
}

pub(super) async fn load_cached_peers(config: &ProxyFloatEscaperConfig) -> anyhow::Result<PeerSet> {
    if let Some(cache_file) = &config.cache_file {
        let records = file::load_peers_from_cache(cache_file).await?;
        super::peer::parse_peers(config, &records)
    } else {
        Ok(PeerSet::default())
    }
}

async fn parse_and_save_peers(
    config: &ProxyFloatEscaperConfig,
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
    config: &ProxyFloatEscaperConfig,
    peers_container: &Arc<ArcSwap<PeerSet>>,
    data: &str,
) -> anyhow::Result<()> {
    let obj = serde_json::from_str(data)
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
) -> anyhow::Result<Option<mpsc::Sender<()>>> {
    let (quit_sender, quit_receiver) = mpsc::channel(1);

    match &config.source {
        ProxyFloatSource::Passive => return Ok(None),
        ProxyFloatSource::Redis(redis) => {
            let redis_job = redis::RedisFetchJob::new(redis)?;
            spawn_job(config, peers_container, redis_job, quit_receiver);
        }
    };

    Ok(Some(quit_sender))
}

fn spawn_job<T>(
    config: Arc<ProxyFloatEscaperConfig>,
    peers_container: Arc<ArcSwap<PeerSet>>,
    fetch_job: T,
    mut quit_receiver: mpsc::Receiver<()>,
) where
    T: FetchJob + Send + 'static,
{
    use mpsc::error::TryRecvError;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(config.refresh_interval);
        interval.tick().await; // will tick immediately
        loop {
            let result = fetch_job.fetch_records().await;
            match result {
                Ok(records) => {
                    match quit_receiver.try_recv() {
                        Ok(_) => break,
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => break,
                    }

                    if let Err(e) = parse_and_save_peers(&config, &peers_container, records).await {
                        warn!("failed to update peers for escaper {}: {e:?}", config.name);
                    }
                }
                Err(e) => warn!("failed to fetch peers for escaper {}: {e:?}", config.name),
            }

            interval.tick().await;
        }
    });
}
