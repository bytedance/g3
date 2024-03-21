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

use std::sync::Arc;

use anyhow::{anyhow, Context};
use openssl::x509::X509;
use rmpv::ValueRef;

use g3_types::net::TlsServiceType;

use super::GeneratedData;

pub(crate) struct Request {
    pub(crate) host: Arc<str>,
    service: TlsServiceType,
    pub(crate) cert: Option<X509>,
}

impl Request {
    pub(crate) fn parse_req(mut data: &[u8]) -> anyhow::Result<Self> {
        let v = rmpv::decode::read_value_ref(&mut data)
            .map_err(|e| anyhow!("invalid req data: {e}"))?;

        if let ValueRef::Map(map) = v {
            let mut host = String::default();
            let mut service = TlsServiceType::Http;
            let mut cert = None;

            for (k, v) in map {
                let key = g3_msgpack::value::as_string(&k)?;
                match g3_msgpack::key::normalize(key.as_str()).as_str() {
                    "host" => {
                        host = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key {key}"))?;
                    }
                    "service" => {
                        service = g3_msgpack::value::as_tls_service_type(&v)
                            .context(format!("invalid tls service type value for key {key}"))?;
                    }
                    "cert" => {
                        let c = g3_msgpack::value::as_openssl_certificate(&v)
                            .context(format!("invalid mimic cert value for key {key}"))?;
                        cert = Some(c);
                    }
                    _ => return Err(anyhow!("invalid key {key}")),
                }
            }

            if host.is_empty() {
                return Err(anyhow!("invalid host value"));
            }
            Ok(Request {
                host: Arc::from(host),
                service,
                cert,
            })
        } else {
            Err(anyhow!("the req root data type should be map"))
        }
    }

    pub(crate) fn encode_rsp(&self, generated: &GeneratedData) -> anyhow::Result<Vec<u8>> {
        let map = vec![
            (
                ValueRef::String("host".into()),
                ValueRef::String(self.host.as_ref().into()),
            ),
            (
                ValueRef::String("service".into()),
                ValueRef::String(self.service.as_str().into()),
            ),
            (
                ValueRef::String("cert".into()),
                ValueRef::String(generated.cert.as_str().into()),
            ),
            (
                ValueRef::String("key".into()),
                ValueRef::Binary(&generated.key),
            ),
            (
                ValueRef::String("ttl".into()),
                ValueRef::Integer(generated.ttl.into()),
            ),
        ];
        let mut buf = Vec::with_capacity(4096);
        let v = ValueRef::Map(map);
        rmpv::encode::write_value_ref(&mut buf, &v)
            .map_err(|e| anyhow!("msgpack encode failed: {e}"))?;
        Ok(buf)
    }
}
