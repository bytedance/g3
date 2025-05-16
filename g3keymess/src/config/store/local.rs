/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::{Path, PathBuf};

use anyhow::anyhow;
use log::{debug, warn};
use openssl::pkey::{PKey, Private};
use tokio::sync::oneshot;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::KeyStoreConfig;

#[derive(Clone, Debug, PartialEq)]
pub struct LocalKeyStoreConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    dir_path: PathBuf,
    watch: bool,
}

impl LocalKeyStoreConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        LocalKeyStoreConfig {
            name: NodeName::default(),
            position,
            dir_path: PathBuf::new(),
            watch: false,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = LocalKeyStoreConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.dir_path.as_os_str().is_empty() {
            return Err(anyhow!("dir path is not set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_STORE_TYPE => Ok(()),
            "name" => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "dir" | "directory" | "dir_path" | "directory_path" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.dir_path = g3_yaml::value::as_dir_path(v, lookup_dir, false)?;
                Ok(())
            }
            "watch" => {
                self.watch = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl KeyStoreConfig for LocalKeyStoreConfig {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    async fn load_keys(&self) -> anyhow::Result<()> {
        const BATCH_SIZE: usize = 128;

        debug!("loading keys from dir {}", self.dir_path.display());
        let mut dir = tokio::fs::read_dir(&self.dir_path)
            .await
            .map_err(|e| anyhow!("failed to open {}: {e}", self.dir_path.display()))?;

        let mut count = 0;
        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| anyhow!("failed to read dir {}: {e}", self.dir_path.display()))?
        {
            if count >= BATCH_SIZE {
                tokio::task::yield_now().await;
                count = 0;
            } else {
                count += 1;
            }

            let path = entry.path();
            let filetype = match entry.file_type().await {
                Ok(t) => t,
                Err(e) => {
                    warn!(
                        " - failed to get filetype for dir entry {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };

            if filetype.is_file() {
                load_add_key(&path).await;
            } else if filetype.is_symlink() {
                // traverse the symlink to get the real file type
                match tokio::fs::metadata(&path).await {
                    Ok(meta) => {
                        if meta.is_file() {
                            load_add_key(&path).await;
                        } else {
                            debug!(" - skip non-regular file {}", path.display());
                        }
                    }
                    Err(e) => {
                        warn!(" - failed to get metadata for {}: {e}", path.display());
                    }
                }
            } else {
                debug!(" - skip non-regular file {}", path.display());
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn spawn_subscriber(&self) -> anyhow::Result<Option<oneshot::Sender<()>>> {
        use std::future::poll_fn;

        use futures_util::StreamExt;
        use inotify::{Inotify, WatchMask};

        if !self.watch {
            return Ok(None);
        }

        let inotify =
            Inotify::init().map_err(|e| anyhow!("failed to init inotify instance: {e}"))?;
        inotify
            .watches()
            .add(&self.dir_path, WatchMask::CLOSE_WRITE)
            .map_err(|e| {
                anyhow!(
                    "failed to watch close_write event of {}: {e}",
                    self.dir_path.display()
                )
            })?;
        let buffer = [0u8; 4096];
        let mut event_stream = inotify.into_event_stream(buffer)?;

        let dir_path = self.dir_path.to_path_buf();
        let async_watch = async move {
            loop {
                match poll_fn(|cx| event_stream.poll_next_unpin(cx)).await {
                    Some(Ok(v)) => {
                        if let Some(p) = v.name {
                            let path = dir_path.join(p);
                            debug!("got close_write event on {}", path.display());
                            load_add_key(&path).await;
                        }
                    }
                    Some(Err(e)) => {
                        warn!("inotify watch failed: {e}");
                    }
                    None => {
                        warn!("inotify watch ended unexpected");
                    }
                }
            }
        };
        let (quit_sender, quit_receiver) = oneshot::channel();

        tokio::spawn(async {
            tokio::select! {
                _ = quit_receiver => {}
                _ = async_watch => {}
            }
        });

        Ok(Some(quit_sender))
    }

    #[cfg(not(target_os = "linux"))]
    fn spawn_subscriber(&self) -> anyhow::Result<Option<oneshot::Sender<()>>> {
        Ok(None)
    }
}

async fn load_add_key(path: &Path) {
    match load_key(path).await {
        Ok(Some(key)) => match crate::store::add_global(key) {
            Ok(_) => {
                debug!(" - loaded key from file {}", path.display());
            }
            Err(e) => {
                warn!(" - failed to add key from file {}: {e}", path.display());
            }
        },
        Ok(None) => {
            debug!(" - no key found in file {}", path.display());
        }
        Err(e) => {
            warn!(" - failed to load key from file {}: {e}", path.display());
        }
    }
}

async fn load_key<T: AsRef<Path>>(path: T) -> anyhow::Result<Option<PKey<Private>>> {
    let path = path.as_ref();
    if let Some(ext) = path.extension() {
        if ext.eq_ignore_ascii_case("key") {
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| anyhow!("failed to read content of file {}: {e}", path.display()))?;
            let key = PKey::private_key_from_pem(content.as_bytes())
                .map_err(|e| anyhow!("invalid private key pem file {}: {e}", path.display()))?;
            return Ok(Some(key));
        }
    }
    Ok(None)
}
