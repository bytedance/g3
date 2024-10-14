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
use chrono::{DateTime, Utc};
use serde_json::Value;

use g3_types::net::OpensslTicketKeyBuilder;

use super::{RemoteDecryptKey, RemoteEncryptKey, RemoteKeys};

impl RemoteEncryptKey {
    pub(super) fn parse_json(value: &Value) -> anyhow::Result<Self> {
        if let Value::Object(map) = value {
            let mut builder = OpensslTicketKeyBuilder::default();
            for (k, v) in map {
                match g3_json::key::normalize(k).as_str() {
                    "name" => g3_json::value::as_bytes(v, &mut builder.name)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "aes" | "aes_key" => g3_json::value::as_bytes(v, &mut builder.aes_key)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "hmac" | "hmac_key" => g3_json::value::as_bytes(v, &mut builder.hmac_key)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "lifetime" => {
                        let lifetime = g3_json::value::as_u32(v)
                            .context(format!("invalid u32 value for key {k}"))?;
                        builder.set_lifetime(lifetime);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            Ok(RemoteEncryptKey {
                key: builder.build(),
            })
        } else {
            Err(anyhow!(
                "json value type for 'openssl ticket key' should be 'map'"
            ))
        }
    }
}

impl RemoteDecryptKey {
    pub(super) fn parse_json(value: &Value) -> anyhow::Result<Self> {
        if let Value::Object(map) = value {
            let mut expire: Option<DateTime<Utc>> = None;
            let mut builder = OpensslTicketKeyBuilder::default();
            for (k, v) in map {
                match g3_json::key::normalize(k).as_str() {
                    "name" => g3_json::value::as_bytes(v, &mut builder.name)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "aes" | "aes_key" => g3_json::value::as_bytes(v, &mut builder.aes_key)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "hmac" | "hmac_key" => g3_json::value::as_bytes(v, &mut builder.hmac_key)
                        .context(format!("invalid bytes value for key {k}"))?,
                    "lifetime" => {}
                    "expire" => {
                        let time = g3_json::value::as_rfc3339_datetime(v)
                            .context(format!("invalid rfc3339 datetime value for key {k}"))?;
                        expire = Some(time);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            match expire {
                Some(expire) => Ok(RemoteDecryptKey {
                    key: builder.build(),
                    expire,
                }),
                None => Err(anyhow!("no expire datetime set")),
            }
        } else {
            Err(anyhow!(
                "json value type for 'openssl ticket key' should be 'map'"
            ))
        }
    }
}

impl RemoteKeys {
    #[allow(dead_code)]
    pub(super) fn parse_json(value: &Value) -> anyhow::Result<Self> {
        if let Value::Object(map) = value {
            let mut enc_key: Option<RemoteEncryptKey> = None;
            let mut dec_keys = Vec::new();
            for (k, v) in map {
                match g3_json::key::normalize(k).as_str() {
                    "enc" | "encrypt" | "enc_key" | "encrypt_key" => {
                        let key = RemoteEncryptKey::parse_json(v)
                            .context(format!("invalid remote encrypt key value for key {k}"))?;
                        enc_key = Some(key);
                    }
                    "dec" | "decrypt" | "dec_keys" | "decrypt_keys" => {
                        if let Value::Array(seq) = v {
                            for (i, v) in seq.iter().enumerate() {
                                let key = RemoteDecryptKey::parse_json(v).context(format!(
                                    "invalid single remote decrypt key value for {k}#{i}"
                                ))?;
                                dec_keys.push(key);
                            }
                        } else {
                            let key = RemoteDecryptKey::parse_json(v).context(format!(
                                "invalid single remote decrypt key value for key {k}"
                            ))?;
                            dec_keys.push(key);
                        }
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
            match enc_key {
                Some(enc_key) => Ok(RemoteKeys {
                    enc: enc_key,
                    dec: dec_keys,
                }),
                None => Err(anyhow!("no encrypt key set")),
            }
        } else {
            Err(anyhow!("json value type for 'remote keys' should be 'map'"))
        }
    }
}
