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

use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::S2nTlsCertPair;

fn as_certificates_from_single_element(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<String> {
    const MAX_FILE_SIZE: usize = 4_000_000; // 4MB

    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            return Ok(s.to_string());
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    Ok(contents)
}

pub fn as_s2n_tls_certificates(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<Vec<String>> {
    if let Yaml::Array(seq) = value {
        let mut certs = Vec::new();
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v, lookup_dir)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.push(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value, lookup_dir).map(|cert| vec![cert])
    }
}

pub fn as_s2n_tls_private_key(value: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<String> {
    const MAX_FILE_SIZE: usize = 256_000; // 256KB

    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            return Ok(s.to_string());
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    Ok(contents)
}

pub fn as_s2n_tls_certificate_pair(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<S2nTlsCertPair> {
    if let Yaml::Hash(map) = value {
        let mut pair = S2nTlsCertPair::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "certificate" | "cert" => {
                let cert = as_certificates_from_single_element(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                pair.set_cert_chain(cert);
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_s2n_tls_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                pair.set_private_key(key);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        pair.check()?;
        Ok(pair)
    } else {
        Err(anyhow!(
            "yaml value type for s2n tls certificate pair should be 'map'"
        ))
    }
}
