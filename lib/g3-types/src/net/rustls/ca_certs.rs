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

use std::fs::File;
use std::io::BufReader;

use anyhow::anyhow;
use rustls::Certificate;

pub fn load_openssl_certs_for_rustls() -> anyhow::Result<Vec<Certificate>> {
    let r = openssl_probe::probe();
    let Some(path) = r.cert_file else {
        return Err(anyhow!("no ca certificate file could be found"));
    };
    let f = File::open(&path)
        .map_err(|e| anyhow!("failed to open ca cert file {}: {e}", path.display()))?;
    let mut f = BufReader::new(f);

    rustls_pemfile::certs(&mut f)
        .map(|v| v.into_iter().map(Certificate).collect())
        .map_err(|e| anyhow!("failed to load pem certs from file {}: {e}", path.display()))
}
