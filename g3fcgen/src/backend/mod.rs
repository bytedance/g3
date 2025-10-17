/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use log::{debug, error, info, warn};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use tokio::runtime::Handle;

use g3_cert_agent::Request;
use g3_tls_cert::builder::{MimicCertBuilder, ServerCertBuilder, TlsServerCertBuilder};
use g3_tls_cert::ext::X509Ext;
use g3_types::net::{Host, TlsCertUsage};

mod stats;
pub(crate) use stats::BackendStats;

use super::{BackendRequest, BackendResponse};
use crate::config::OpensslBackendConfig;
use crate::frontend::GeneratedData;

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
        if let Some(mimic_cert) = req.cert() {
            self.generate_mimic(mimic_cert, req.cert_usage())
        } else {
            let host = Host::from_str(req.host_str())?;
            self.builder.refresh_serial()?;
            let cert =
                self.builder
                    .build_fake(&host, &self.config.ca_cert, &self.config.ca_key, None)?;
            let ttl = self.builder.valid_seconds()?;
            self.pack_data(cert, self.builder.pkey(), ttl)
        }
    }

    fn generate_mimic(
        &self,
        mimic_cert: &X509,
        cert_usage: TlsCertUsage,
    ) -> anyhow::Result<GeneratedData> {
        let mut mimic_builder = MimicCertBuilder::new(mimic_cert)?;
        mimic_builder.set_keep_serial(self.config.keep_serial);

        let cert = match cert_usage {
            TlsCertUsage::TlsServer => {
                mimic_builder.build_tls_cert(&self.config.ca_cert, &self.config.ca_key, None)?
            }
            TlsCertUsage::TLsServerTongsuo => mimic_builder.build_tls_cert_with_new_usage(
                &self.config.ca_cert,
                &self.config.ca_key,
                None,
            )?,
            TlsCertUsage::TlcpServerEncryption => mimic_builder.build_tlcp_enc_cert(
                &self.config.ca_cert,
                &self.config.ca_key,
                None,
            )?,
            TlsCertUsage::TlcpServerSignature => mimic_builder.build_tlcp_sign_cert(
                &self.config.ca_cert,
                &self.config.ca_key,
                None,
            )?,
        };

        let ttl = mimic_builder.valid_seconds()?;

        self.pack_data(cert, mimic_builder.pkey(), ttl)
    }

    fn pack_data(
        &self,
        cert: X509,
        pkey: &PKey<Private>,
        ttl: i32,
    ) -> anyhow::Result<GeneratedData> {
        let ttl = ttl.clamp(0, self.config.max_ttl) as u32;
        let mut cert_pem = cert
            .to_pem()
            .map_err(|e| anyhow!("failed to encode cert to PEM format: {e}"))?;
        if !self.config.ca_cert_pem.is_empty() {
            cert_pem.extend_from_slice(&self.config.ca_cert_pem);
        }
        let key = pkey
            .private_key_to_der()
            .map_err(|e| anyhow!("failed to encode pkey to DER format: {e}"))?;

        let data = GeneratedData {
            cert: unsafe { String::from_utf8_unchecked(cert_pem) },
            key,
            ttl,
        };
        self.stats.add_request_ok();
        Ok(data)
    }

    pub(crate) fn spawn(
        mut self,
        handle: &Handle,
        id: usize,
        req_receiver: kanal::AsyncReceiver<BackendRequest>,
        rsp_sender: kanal::AsyncSender<BackendResponse>,
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
                    r = req_receiver.recv() => {
                        let Ok(req) = r else {
                            break
                        };

                        if let Some(cert) = req.user_req.cert() {
                            info!("{}", cert.dump());
                        }

                        let host = req.user_req.host();
                        debug!("{host} - [#{id}] start cert generation");
                        match self.generate(&req.user_req) {
                            Ok(data) => {
                                debug!("{host} - [#{id}] cert generated");
                                if let Err(e) = rsp_sender.send(req.into_response(data)).await {
                                    error!("{host} - [#{id}] failed to send cert to frontend: {e}");
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("{host} - [#{id}] cert generation failed: {e:?}");
                            }
                        }
                    }
                }
            }
        });
    }
}
