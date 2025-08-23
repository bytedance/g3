/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::fs::DirBuilder;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

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
        if listen_path.exists() {
            return Err(anyhow!(
                "control socket path {} already exists",
                listen_path.display()
            ));
        }
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
        if listen_path.exists() {
            std::fs::remove_file(&listen_path)
                .map_err(|e| anyhow!("failed to remove old {}: {e}", listen_path.display()))?;
        }
        check_then_finalize_path(&listen_path)?;

        debug!("setting up daemon controller {}", listen_path.display());
        let controller = LocalControllerImpl::new(listen_path)?;
        debug!("daemon controller created");
        Ok(controller)
    }

    pub(super) async fn connect_to_daemon(
        _daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<impl AsyncRead + AsyncWrite + use<>> {
        let socket_name = format!("{daemon_group}.sock");
        let mut socket_path = crate::opts::control_dir();
        socket_path.push(Path::new(&socket_name));

        tokio::net::UnixStream::connect(&socket_path)
            .await
            .map_err(|e| {
                anyhow!(
                    "failed to connect to control socket {}: {e:?}",
                    socket_path.display()
                )
            })
    }

    pub(super) async fn into_running(
        self,
        mut quit_receiver: oneshot::Receiver<oneshot::Sender<Self>>,
    ) {
        loop {
            tokio::select! {
                biased;

                r = self.listener.accept() => {
                    match r {
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
                        }
                    }
                }
                r = &mut quit_receiver => {
                    if let Ok(v) = r {
                        let _ = v.send(self);
                    }
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
