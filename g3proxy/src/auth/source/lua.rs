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

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use log::warn;
use mlua::{Function, Lua, Value};

use crate::config::auth::source::lua::UserDynamicLuaSource;
use crate::config::auth::UserConfig;

pub(super) async fn fetch_records(
    source: &Arc<UserDynamicLuaSource>,
) -> anyhow::Result<Vec<UserConfig>> {
    let contents = tokio::time::timeout(
        source.fetch_timeout,
        call_lua_fetch(source.fetch_script.clone()),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "timed out to run lua fetch script {}",
            source.fetch_script.display()
        )
    })??;

    match parse_content(&source.fetch_script, &contents) {
        Ok(all_config) => {
            if let Some(script) = &source.report_script {
                match tokio::time::timeout(
                    source.report_timeout,
                    call_lua_report_ok(script.clone()),
                )
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        warn!(
                            "failed to run lua reportOk function in script {}: {e:?}",
                            script.display()
                        );
                    }
                    Err(_) => {
                        warn!(
                            "timed out to run lua reportOk function in script {}",
                            script.display()
                        )
                    }
                }
            }

            // we should avoid corrupt write at process exit
            if let Some(Err(e)) =
                crate::control::run_protected_io(tokio::fs::write(&source.cache_file, contents))
                    .await
            {
                warn!("failed to cache dynamic users to file {} ({e:?}), this may lead to auth error during restart",
                    source.cache_file.display());
            }

            Ok(all_config)
        }
        Err(e) => {
            if let Some(script) = &source.report_script {
                match tokio::time::timeout(
                    source.report_timeout,
                    call_lua_report_err(script.clone(), format!("{e:?}")),
                )
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        warn!(
                            "failed to run lua reportErr method in script {}: {e:?}",
                            script.display()
                        );
                    }
                    Err(_) => {
                        warn!(
                            "timed out to run lua reportErr function in script {}",
                            script.display()
                        )
                    }
                }
            }

            Err(e)
        }
    }
}

fn parse_content(script: &Path, content: &str) -> anyhow::Result<Vec<UserConfig>> {
    let doc = serde_json::Value::from_str(content).map_err(|e| {
        anyhow!(
            "response from script {} is not valid json: {e}",
            script.display(),
        )
    })?;

    crate::config::auth::source::cache::parse_json(&doc)
}

async fn call_lua_fetch(script: PathBuf) -> anyhow::Result<String> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        let lua = unsafe { Lua::unsafe_new() };
        let code = lua.load(&code);
        code.eval::<String>()
            .map_err(|e| anyhow!("failed to run lua fetch script {}: {e}", script.display()))
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}

async fn call_lua_report_ok(script: PathBuf) -> anyhow::Result<()> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        let lua = unsafe { Lua::unsafe_new() };
        lua.load(&code)
            .exec()
            .map_err(|e| anyhow!("failed to load lua report script {}: {e}", script.display()))?;

        let report_ok = lua
            .globals()
            .get::<_, Function>("reportOk")
            .map_err(|e| anyhow!("failed to load reportOk function: {e}"))?;
        report_ok
            .call::<_, ()>(Value::Nil)
            .map_err(|e| anyhow!("failed to call reportOk: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}

async fn call_lua_report_err(script: PathBuf, e: String) -> anyhow::Result<()> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        let lua = unsafe { Lua::unsafe_new() };
        lua.load(&code)
            .exec()
            .map_err(|e| anyhow!("failed to load lua report script {}: {e}", script.display()))?;

        let report_err = lua
            .globals()
            .get::<_, Function>("reportErr")
            .map_err(|e| anyhow!("failed to load reportErr function: {e}"))?;
        report_err
            .call::<_, ()>(e)
            .map_err(|e| anyhow!("failed to call reportErr: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}
