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

use anyhow::{anyhow, Context};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rmpv::ValueRef;

fn as_certificates_from_single_element(value: &ValueRef) -> anyhow::Result<Vec<X509>> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type 'certificates' should be 'string' or 'binary'"
            ))
        }
    };

    let certs =
        X509::stack_from_pem(bytes).map_err(|e| anyhow!("invalid certificate string: {e}"))?;
    if certs.is_empty() {
        Err(anyhow!("no valid certificate found"))
    } else {
        Ok(certs)
    }
}

pub fn as_openssl_certificates(value: &ValueRef) -> anyhow::Result<Vec<X509>> {
    if let ValueRef::Array(seq) = value {
        let mut certs = Vec::new();
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.extend(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value)
    }
}

pub fn as_openssl_certificate(value: &ValueRef) -> anyhow::Result<X509> {
    match value {
        ValueRef::String(s) => {
            X509::from_pem(s.as_bytes()).map_err(|e| anyhow!("invalid PEM encoded x509 cert: {e}"))
        }
        ValueRef::Binary(b) => {
            X509::from_der(b).map_err(|e| anyhow!("invalid DER encoded x509 cert: {e}"))
        }
        _ => Err(anyhow!(
            "msgpack value for 'certificate' should be 'pem string' or 'der binary'"
        )),
    }
}

pub fn as_openssl_private_key(value: &ValueRef) -> anyhow::Result<PKey<Private>> {
    match value {
        ValueRef::String(s) => PKey::private_key_from_pem(s.as_bytes())
            .map_err(|e| anyhow!("invalid PEM encoded private key: {e}")),
        ValueRef::Binary(b) => PKey::private_key_from_der(b)
            .map_err(|e| anyhow!("invalid DER encoded private key: {e}")),
        _ => Err(anyhow!(
            "msgpack value type for 'private key' should be 'pem string' or 'der binary'"
        )),
    }
}
