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

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use log::warn;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::config::auth::source::python::UserDynamicPythonSource;
use crate::config::auth::UserConfig;

const FN_NAME_FETCH_USERS: &str = "fetch_users";
const FN_NAME_REPORT_OK: &str = "report_ok";
const FN_NAME_REPORT_ERR: &str = "report_err";

pub(super) async fn fetch_records(
    source: &Arc<UserDynamicPythonSource>,
) -> anyhow::Result<Vec<UserConfig>> {
    let contents = tokio::time::timeout(
        source.fetch_timeout,
        call_python_fetch(source.script_file.clone()),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "timed out to run {FN_NAME_FETCH_USERS} function in script {}",
            source.script_file.display()
        )
    })??;

    match parse_content(source, &contents) {
        Ok(all_config) => {
            match tokio::time::timeout(
                source.report_timeout,
                call_python_report_ok(source.script_file.clone()),
            )
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    warn!(
                        "failed to run {FN_NAME_REPORT_OK} function in script {}: {e:?}",
                        source.script_file.display()
                    );
                }
                Err(_) => {
                    warn!(
                        "timed out to run {FN_NAME_REPORT_OK} function in script {}",
                        source.script_file.display()
                    )
                }
            }

            // we should avoid corrupt write at process exit
            if let Some(Err(e)) =
                crate::control::run_protected_io(tokio::fs::write(&source.cache_file, contents))
                    .await
            {
                warn!(
                    "failed to cache dynamic users to file {} ({e:?}), this may lead to auth error during restart",
                    source.cache_file.display()
                );
            }

            Ok(all_config)
        }
        Err(e) => {
            match tokio::time::timeout(
                source.report_timeout,
                call_python_report_err(source.script_file.clone(), format!("{e:?}")),
            )
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    warn!(
                        "failed to run {FN_NAME_REPORT_ERR} function in script {}: {e:?}",
                        source.script_file.display()
                    );
                }
                Err(_) => {
                    warn!(
                        "timed out to run {FN_NAME_REPORT_ERR} function in script {}",
                        source.script_file.display()
                    )
                }
            }

            Err(e)
        }
    }
}

fn parse_content(
    source: &UserDynamicPythonSource,
    content: &str,
) -> anyhow::Result<Vec<UserConfig>> {
    let doc = serde_json::Value::from_str(content).map_err(|e| {
        anyhow!(
            "response from {}::{FN_NAME_FETCH_USERS}() is not valid json: {e}",
            source.script_file.display(),
        )
    })?;

    crate::config::auth::source::cache::parse_json(&doc)
}

async fn call_python_fetch(script: PathBuf) -> anyhow::Result<String> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let code = PyModule::from_code(py, code.as_str(), "", "").map_err(|e| {
                anyhow!(
                    "failed to load code from script file {}: {e:?}",
                    script.display(),
                )
            })?;

            let fetch_users = code.getattr(FN_NAME_FETCH_USERS).map_err(|e| {
                anyhow!(
                    "no {FN_NAME_FETCH_USERS} function found in script {}: {e:?}",
                    script.display(),
                )
            })?;

            let result: String = fetch_users
                .call0()
                .map_err(|e| {
                    anyhow!(
                        "failed to call {}::{FN_NAME_FETCH_USERS}(): {e:?}",
                        script.display()
                    )
                })?
                .extract()
                .map_err(|e| {
                    anyhow!(
                        "failed to extract string value from {}::{FN_NAME_FETCH_USERS}(): {e:?}",
                        script.display(),
                    )
                })?;

            Ok(result)
        })
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}

async fn call_python_report_ok(script: PathBuf) -> anyhow::Result<()> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let code = PyModule::from_code(py, code.as_str(), "", "").map_err(|e| {
                anyhow!(
                    "failed to load code from script file {}: {e:?}",
                    script.display(),
                )
            })?;

            if let Ok(report_ok) = code.getattr(FN_NAME_REPORT_OK) {
                report_ok.call0().map_err(|e| {
                    anyhow!(
                        "failed to call {}::{FN_NAME_REPORT_OK}(): {e:?}",
                        script.display()
                    )
                })?;
            }

            Ok(())
        })
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}

async fn call_python_report_err(script: PathBuf, e: String) -> anyhow::Result<()> {
    let code = tokio::fs::read_to_string(&script).await.map_err(|e| {
        anyhow!(
            "failed to read in content of file {}: {e}",
            script.display(),
        )
    })?;

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let code = PyModule::from_code(py, code.as_str(), "", "").map_err(|e| {
                anyhow!(
                    "failed to load code from script file {}: {e:?}",
                    script.display(),
                )
            })?;

            if let Ok(report_ok) = code.getattr(FN_NAME_REPORT_ERR) {
                let tup = PyTuple::new(py, [e]);
                report_ok.call1(tup).map_err(|e| {
                    anyhow!(
                        "failed to call {}::{FN_NAME_REPORT_ERR}(err_msg): {e:?}",
                        script.display()
                    )
                })?;
            }

            Ok(())
        })
    })
    .await
    .map_err(|e| anyhow!("join blocking task error: {e}"))?
}
