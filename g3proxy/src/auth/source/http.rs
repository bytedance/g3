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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use curl::easy::Easy;
use log::warn;

use crate::config::auth::source::http::UserDynamicHttpSource;
use crate::config::auth::UserConfig;

pub(super) async fn fetch_records(
    source: &Arc<UserDynamicHttpSource>,
) -> anyhow::Result<Vec<UserConfig>> {
    let source2 = Arc::clone(source);
    let contents = tokio::task::spawn_blocking(move || call_curl_blocking(source2))
        .await
        .map_err(|e| anyhow!("join blocking task error: {e}"))?
        .context(format!("unable to fetch url {}", source.url))?;

    let doc = serde_json::Value::from_str(&contents).map_err(|e| {
        if contents.len() >= source.max_body_size {
            anyhow!(
                "response is not valid json as the max body size {} reached",
                source.max_body_size
            )
        } else {
            anyhow!("response is not valid json: {e}")
        }
    })?;

    let all_config = crate::config::auth::source::cache::parse_json(&doc)?;

    // we should avoid corrupt write at process exit
    if let Some(Err(e)) =
        crate::control::run_protected_io(tokio::fs::write(&source.cache_file, contents)).await
    {
        warn!(
            "failed to cache dynamic users to file {} ({e:?}), this may lead to auth error during restart",
            source.cache_file.display()
        );
    }

    Ok(all_config)
}

fn call_curl_blocking(source: Arc<UserDynamicHttpSource>) -> anyhow::Result<String> {
    let mut easy = Easy::new();
    easy.timeout(source.timeout)
        .map_err(|e| anyhow!("failed to set timeout: {e}"))?;
    easy.connect_timeout(source.connect_timeout)
        .map_err(|e| anyhow!("failed to set connect timeout: {e}"))?;
    // enable fail on error, in such case response >= 400 will cause return without reading of body
    easy.fail_on_error(true)
        .map_err(|e| anyhow!("failed to enable fail_on_error: {e}"))?;
    if !source.interface.is_empty() {
        easy.interface(&source.interface)
            .map_err(|e| anyhow!("failed to set bind ip: {e}"))?;
    }
    easy.url(source.url.as_str())
        .map_err(|e| anyhow!("failed to set url: {e}"))?;

    let mut buf = Vec::new();

    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                if buf.len() + data.len() > source.max_body_size {
                    // this will return an error with is_write_error
                    Ok(0)
                } else {
                    buf.extend_from_slice(data);
                    Ok(data.len())
                }
            })
            .map_err(|e| anyhow!("failed to set write callback function: {e}"))?;
        transfer
            .perform()
            .map_err(|e| anyhow!("curl failed: {e}"))?;
    }

    String::from_utf8(buf).map_err(|e| anyhow!("the response body is not utf-8 encoded: {e}"))
}
