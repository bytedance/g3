/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use log::{debug, warn};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub(super) struct LocalControllerImpl {
    pipe_name: String,
    server: NamedPipeServer,
}

impl LocalControllerImpl {
    fn create(pipe_name: String) -> anyhow::Result<Self> {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(2)
            .create(&pipe_name)?;
        Ok(LocalControllerImpl { pipe_name, server })
    }

    pub(super) fn listen_path(&self) -> String {
        self.pipe_name.clone()
    }

    pub(super) fn create_unique(daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let pipe_name = format!(
            r"\\.\pipe\{daemon_name}@{daemon_group}:{}",
            std::process::id()
        );
        LocalControllerImpl::create(pipe_name)
    }

    pub(super) fn create_daemon(daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let pipe_name = format!(r"\\.\pipe\{daemon_name}@{daemon_group}");
        LocalControllerImpl::create(pipe_name)
    }

    pub(super) async fn into_running(mut self) {
        loop {
            match self.server.connect().await {
                Ok(_) => {
                    debug!("new ctl local control client");
                    match ServerOptions::new().create(&self.pipe_name) {
                        Ok(new_server) => {
                            let server = std::mem::replace(&mut self.server, new_server);
                            let (r, w) = tokio::io::split(server);
                            super::ctl_handle(r, w);
                        }
                        Err(e) => {
                            warn!("failed to re-open controller pipe {}: {e}", self.pipe_name);
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("controller {} accept: {e}", self.pipe_name);
                    break;
                }
            }
        }
    }
}
