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

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use yaml_rust::Yaml;

use g3_histogram::Quantile;

static BACKEND_CONFIG_LOCK: OnceLock<Arc<OpensslBackendConfig>> = OnceLock::new();

pub(crate) fn get_config() -> Option<Arc<OpensslBackendConfig>> {
    BACKEND_CONFIG_LOCK.get().cloned()
}

pub(crate) struct OpensslBackendConfig {
    pub(crate) ca_cert: X509,
    pub(crate) ca_key: PKey<Private>,
    pub(crate) ca_cert_pem: Vec<u8>,
    pub(crate) request_duration_quantile: BTreeSet<Quantile>,
    pub(crate) request_duration_rotate: Duration,
}

pub(super) fn load_config(value: &Yaml) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        let mut no_append_ca_cert = false;
        let mut ca_cert_pem = Vec::new();
        let mut ca_cert: Option<X509> = None;
        let mut ca_key: Option<PKey<Private>> = None;
        let mut request_duration_quantile = BTreeSet::new();
        let mut request_duration_rotate = Duration::from_secs(4);
        let lookup_dir = g3_daemon::config::get_lookup_dir(None)?;

        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "ca_certificate" => {
                let certs = g3_yaml::value::as_openssl_certificates(v, Some(lookup_dir))
                    .context(format!("invalid openssl certificate value for key {k}"))?;
                for (i, cert) in certs.iter().enumerate() {
                    let pem = cert.to_pem().map_err(|e| {
                        anyhow!("failed to convert cert {i} back to pem format: {e}")
                    })?;
                    ca_cert_pem.extend(pem);
                }

                let cert = certs
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("no valid openssl certificate key found"))?;
                ca_cert = Some(cert);
                Ok(())
            }
            "ca_private_key" => {
                let key = g3_yaml::value::as_openssl_private_key(v, Some(lookup_dir))
                    .context(format!("invalid openssl private key value for key {k}"))?;
                ca_key = Some(key);
                Ok(())
            }
            "no_append_ca_cert" => {
                no_append_ca_cert = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "request_duration_quantile" => {
                request_duration_quantile = g3_yaml::value::as_quantile_list(v)?;
                Ok(())
            }
            "request_duration_rotate" => {
                request_duration_rotate = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        let Some(ca_cert) = ca_cert else {
            return Err(anyhow!("no ca certificate set"));
        };
        let Some(ca_key) = ca_key else {
            return Err(anyhow!("no ca private key set"));
        };

        if no_append_ca_cert {
            ca_cert_pem.clear();
        }
        BACKEND_CONFIG_LOCK
            .set(Arc::new(OpensslBackendConfig {
                ca_cert,
                ca_key,
                ca_cert_pem,
                request_duration_quantile,
                request_duration_rotate,
            }))
            .map_err(|_| anyhow!("duplicate backend config"))?;
        Ok(())
    } else {
        Err(anyhow!(
            "yam value type for the backend config should be 'map'"
        ))
    }
}
