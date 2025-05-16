/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;
use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

pub(super) async fn load_peers_from_cache(cache_file: &Path) -> anyhow::Result<Vec<Value>> {
    let contents = tokio::fs::read_to_string(cache_file).await.map_err(|e| {
        anyhow!(
            "failed to read content of cache file {}: {e:?}",
            cache_file.display()
        )
    })?;
    if contents.is_empty() {
        return Ok(Vec::new());
    }
    let doc = serde_json::Value::from_str(&contents).map_err(|e| {
        anyhow!(
            "invalid json content for cache file {}: {e:?}",
            cache_file.display()
        )
    })?;
    match doc {
        Value::Array(seq) => Ok(seq),
        _ => Ok(vec![doc]),
    }
}

pub(super) async fn save_peers_to_cache(
    cache_file: &Path,
    peers: Vec<Value>,
) -> anyhow::Result<()> {
    let doc = Value::Array(peers);
    let content = serde_json::to_string_pretty(&doc)
        .map_err(|e| anyhow!("failed to encoding peer records as json string: {e:?}"))?;
    if let Some(executed) =
        crate::control::run_protected_io(tokio::fs::write(cache_file, content)).await
    {
        executed.map_err(|e| {
            anyhow!(
                "failed to write to cache file {}: {e:?}",
                cache_file.display()
            )
        })?
    }
    Ok(())
}
