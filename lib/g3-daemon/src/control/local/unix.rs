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

use std::fs::DirBuilder;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use log::{debug, warn};
use tokio::net::UnixListener;

pub(super) struct LocalControllerImpl {
    listen_path: PathBuf,
    listener: UnixListener,
}

impl LocalControllerImpl {
    fn new(listen_path: PathBuf) -> io::Result<Self> {
        let listener = UnixListener::bind(&listen_path)?;
        Ok(LocalControllerImpl {
            listen_path,
            listener,
        })
    }

    pub(super) fn listen_path(&self) -> String {
        self.listen_path.display().to_string()
    }

    pub(super) fn create_unique(_daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let socket_name = format!("{daemon_group}_{}.sock", std::process::id());
        let mut listen_path = crate::opts::control_dir();
        listen_path.push(Path::new(&socket_name));
        check_then_finalize_path(&listen_path)?;

        debug!("setting up unique controller {}", listen_path.display());
        let controller = LocalControllerImpl::new(listen_path)?;
        debug!("unique controller created");
        Ok(controller)
    }

    pub(super) fn create_daemon(_daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let socket_name = if daemon_group.is_empty() {
            "_.sock".to_string()
        } else {
            format!("{daemon_group}.sock")
        };
        let mut listen_path = crate::opts::control_dir();
        listen_path.push(Path::new(&socket_name));
        check_then_finalize_path(&listen_path)?;

        debug!("setting up daemon controller {}", listen_path.display());
        let controller = LocalControllerImpl::new(listen_path)?;
        debug!("daemon controller created");
        Ok(controller)
    }

    pub(super) async fn into_running(self) {
        loop {
            let result = self.listener.accept().await;
            match result {
                Ok((stream, addr)) => {
                    if let Ok(ucred) = stream.peer_cred() {
                        if let Some(addr) = addr.as_pathname() {
                            debug!(
                                "new ctl client from {} uid {} pid {}",
                                addr.display(),
                                ucred.uid(),
                                ucred.gid(),
                            );
                        } else {
                            debug!(
                                "new ctl client from uid {} pid {}",
                                ucred.uid(),
                                ucred.gid()
                            );
                        }
                    } else {
                        debug!("new ctl local control client");
                    }

                    let (r, w) = stream.into_split();
                    super::ctl_handle(r, w);
                }
                Err(e) => {
                    warn!("controller {} accept: {e}", self.listen_path.display());
                    break;
                }
            }
        }
    }
}

impl Drop for LocalControllerImpl {
    fn drop(&mut self) {
        if self.listen_path.exists() {
            debug!("unlink socket file {}", self.listen_path.display());
            let _ = std::fs::remove_file(&self.listen_path);
        }
    }
}

fn check_then_finalize_path(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        return Err(anyhow!(
            "control socket path {} already exists",
            path.display()
        ));
    }
    if !path.has_root() {
        return Err(anyhow!(
            "control socket path {} is not absolute",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        debug!("creating control directory {}", parent.display());
        DirBuilder::new().recursive(true).create(parent)?;
    }

    Ok(())
}
