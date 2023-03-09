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

use std::io::{BufRead, BufReader};

use anyhow::{anyhow, Context};
use rmpv::ValueRef;
use rustls::{Certificate, PrivateKey};
use rustls_pemfile::Item;

fn as_certificates_from_single_element(value: &ValueRef) -> anyhow::Result<Vec<Certificate>> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type 'certificates' should be 'string' or 'binary'"
            ));
        }
    };
    let certs = rustls_pemfile::certs(&mut BufReader::new(bytes))
        .map_err(|e| anyhow!("invalid certificate string: {e:?}"))?;
    if certs.is_empty() {
        Err(anyhow!("no valid certificate found"))
    } else {
        Ok(certs.into_iter().map(Certificate).collect())
    }
}

pub fn as_certificates(value: &ValueRef) -> anyhow::Result<Vec<Certificate>> {
    if let ValueRef::Array(seq) = value {
        let mut certs = Vec::with_capacity(seq.len());
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

fn read_first_private_key<R>(reader: &mut R) -> anyhow::Result<PrivateKey>
where
    R: BufRead,
{
    loop {
        match rustls_pemfile::read_one(reader)
            .map_err(|e| anyhow!("read private key failed: {e:?}"))?
        {
            Some(Item::PKCS8Key(d)) => return Ok(PrivateKey(d)),
            Some(Item::RSAKey(d)) => return Ok(PrivateKey(d)),
            Some(Item::ECKey(d)) => return Ok(PrivateKey(d)),
            Some(_) => continue,
            None => return Err(anyhow!("no valid private key found")),
        }
    }
}

pub fn as_private_key(value: &ValueRef) -> anyhow::Result<PrivateKey> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type for 'private key' should be 'string' or 'binary'"
            ));
        }
    };
    read_first_private_key(&mut BufReader::new(bytes))
}
