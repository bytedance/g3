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

use anyhow::anyhow;
use rustls_pki_types::CertificateDer;

pub fn load_native_certs_for_rustls() -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let mut r = rustls_native_certs::load_native_certs();
    if r.certs.is_empty() {
        match r.errors.pop() {
            Some(e) => Err(anyhow!("no certs loaded, the first error: {e}")),
            None => Err(anyhow!("no certs loaded, and no error reported")),
        }
    } else {
        Ok(r.certs)
    }
}
