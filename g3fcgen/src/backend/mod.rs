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
use std::time::Duration;

use anyhow::anyhow;
use flume::{Receiver, Sender};
use log::{debug, error, warn};
use tokio::runtime::Handle;

use g3_tls_cert::builder::{ServerCertBuilder, TlsServerCertBuilder};
use g3_types::net::Host;

mod stats;
pub(crate) use stats::BackendStats;

use super::{BackendRequest, BackendResponse};
use crate::config::OpensslBackendConfig;
use crate::frontend::{GeneratedData, Request};

pub(crate) struct OpensslBackend {
    config: Arc<OpensslBackendConfig>,
    builder: ServerCertBuilder,
    stats: Arc<BackendStats>,
}

impl OpensslBackend {
    pub(crate) fn new(
        config: &Arc<OpensslBackendConfig>,
        stats: &Arc<BackendStats>,
    ) -> anyhow::Result<Self> {
        let builder = TlsServerCertBuilder::new_ec256()?;
        Ok(OpensslBackend {
            config: Arc::clone(config),
            builder,
            stats: Arc::clone(stats),
        })
    }

    pub(crate) fn refresh(&mut self) -> anyhow::Result<()> {
        self.stats.add_refresh_total();
        self.builder.refresh_datetime()?;
        self.builder.refresh_ec256()?;
        self.stats.add_refresh_ok();
        Ok(())
    }

    fn generate(&mut self, req: &Request) -> anyhow::Result<GeneratedData> {
        self.stats.add_request_total();
        let host = Host::from_str(req.host.as_ref())?;
        self.builder.refresh_serial()?;
        let cert = if let Some(mimic_cert) = &req.cert {
            self.builder
                .build_mimic(mimic_cert, &self.config.ca_cert, &self.config.ca_key, None)?
        } else {
            self.builder
                .build_fake(&host, &self.config.ca_cert, &self.config.ca_key, None)?
        };
        let mut cert_pem = cert
            .to_pem()
            .map_err(|e| anyhow!("failed to encode cert: {e}"))?;
        if !self.config.ca_cert_pem.is_empty() {
            cert_pem.extend_from_slice(&self.config.ca_cert_pem);
        }
        let key_pem = self
            .builder
            .pkey()
            .private_key_to_pem_pkcs8()
            .map_err(|e| anyhow!("failed to encode pkey: {e}"))?;

        let data = GeneratedData {
            cert: unsafe { String::from_utf8_unchecked(cert_pem) },
            key: unsafe { String::from_utf8_unchecked(key_pem) },
            ttl: 300,
        };
        self.stats.add_request_ok();
        Ok(data)
    }

    pub(crate) fn spawn(
        mut self,
        handle: &Handle,
        id: usize,
        req_receiver: Receiver<BackendRequest>,
        rsp_sender: Sender<BackendResponse>,
    ) {
        handle.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self.refresh() {
                            warn!("failed to refresh backend: {e:?}");
                        }
                    }
                    r = req_receiver.recv_async() => {
                        let Ok(req) = r else {
                            break
                        };

                        let host = req.user_req.host.clone();
                        match self.generate(&req.user_req) {
                            Ok(data) => {
                                debug!("Worker#{id} got certificate for host {host}");
                                if let Err(e) = rsp_sender.send_async(req.into_response(data)).await {
                                    error!(
                                        "Worker#{id} failed to send certificate for host {host} to frontend: {e}"
                                    );
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Worker#{id} generate for {host} failed: {e:?}");
                            }
                        }
                    }
                }
            }
        });
    }
}
