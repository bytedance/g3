/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    // Helper function to create valid JSON values for testing
    fn valid_encrypt_key_json() -> Value {
        json!({
            "name": "746573745f6b65790000000000000000", // "test_key" padded to 16 bytes in hex
            "aes_key": "6161616161616161616161616161616161616161616161616161616161616161", // 32 bytes of 'a'
            "hmac_key": "68686868686868686868686868686868", // 16 bytes of 'h'
            "lifetime": 3600
        })
    }

    fn valid_decrypt_key_json() -> Value {
        json!({
            "name": "6f6c645f6b6579000000000000000000", // "old_key" padded to 16 bytes in hex
            "aes": "6262626262626262626262626262626262626262626262626262626262626262", // 32 bytes of 'b'
            "hmac": "69696969696969696969696969696969", // 16 bytes of 'i'
            "lifetime": 9999,
            "expire": "2025-12-31T23:59:59Z"
        })
    }

    fn valid_remote_keys_json() -> Value {
        json!({
            "encrypt": valid_encrypt_key_json(),
            "decrypt": [valid_decrypt_key_json()]
        })
    }

    #[test]
    fn remote_encrypt_key_parse_json() {
        // Valid case
        let json_value = valid_encrypt_key_json();
        let result = RemoteEncryptKey::parse_json(&json_value);
        assert!(result.is_ok());

        let encrypt_key = result.unwrap();
        assert_eq!(
            encrypt_key.key.name().as_ref(),
            b"test_key\x00\x00\x00\x00\x00\x00\x00\x00"
        );

        // Invalid name type
        let json_value = json!({
            "name": 123,
        });
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        // Invalid aes_key type
        let json_value = json!({
            "aes": -123,
        });
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        // Invalid hmac_key type
        let json_value = json!({
            "hmac": true,
        });
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        // Invalid lifetime type
        let json_value = json!({
            "lifetime": "invalid"
        });
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        // Invalid key
        let json_value = json!({
            "unknown_field": "value"
        });
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        // Non-object JSON
        let json_value = json!("not an object");
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        let json_value = json!([1, 2, 3]);
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());

        let json_value = json!(null);
        assert!(RemoteEncryptKey::parse_json(&json_value).is_err());
    }

    #[test]
    fn remote_decrypt_key_parse_json() {
        // Valid case
        let json_value = valid_decrypt_key_json();
        let result = RemoteDecryptKey::parse_json(&json_value);
        assert!(result.is_ok());

        let decrypt_key = result.unwrap();
        assert_eq!(
            decrypt_key.key.name().as_ref(),
            b"old_key\x00\x00\x00\x00\x00\x00\x00\x00\x00"
        );
        assert_eq!(
            decrypt_key.expire,
            Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap()
        );

        // Missing expire field
        let json_value = json!({
            "name": "6f6c645f6b6579000000000000000000",
            "aes_key": "6262626262626262626262626262626262626262626262626262626262626262",
            "hmac_key": "69696969696969696969696969696969"
        });
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());

        // Invalid name type
        let json_value = json!({
            "name": 456,
        });
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());

        // Invalid aes_key type
        let json_value = json!({
            "aes_key": -456,
        });
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());

        // Invalid hmac_key type
        let json_value = json!({
            "hmac_key": false,
        });
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());

        // Invalid expire format
        let json_value = json!({
            "expire": "invalid-date-format"
        });
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());

        // Non-object JSON
        let json_value = json!(42);
        assert!(RemoteDecryptKey::parse_json(&json_value).is_err());
    }

    #[test]
    fn remote_keys_parse_json() {
        // Valid case
        let json_value = valid_remote_keys_json();
        let result = RemoteKeys::parse_json(&json_value);
        assert!(result.is_ok());

        let remote_keys = result.unwrap();
        assert_eq!(
            remote_keys.enc.key.name().as_ref(),
            b"test_key\x00\x00\x00\x00\x00\x00\x00\x00"
        );
        assert_eq!(remote_keys.dec.len(), 1);
        assert_eq!(
            remote_keys.dec[0].key.name().as_ref(),
            b"old_key\x00\x00\x00\x00\x00\x00\x00\x00\x00"
        );

        // Different field name variants
        let json_value = json!({
            "enc": valid_encrypt_key_json(),
            "dec": valid_decrypt_key_json()
        });
        assert!(RemoteKeys::parse_json(&json_value).is_ok());

        let json_value = json!({
            "encrypt_key": valid_encrypt_key_json(),
            "decrypt_keys": [valid_decrypt_key_json()]
        });
        assert!(RemoteKeys::parse_json(&json_value).is_ok());

        // Decrypt key as array
        let json_value = json!({
            "encrypt": valid_encrypt_key_json(),
            "decrypt": [
                valid_decrypt_key_json(),
                {
                    "name": "6b657932000000000000000000000000", // "key2" padded to 16 bytes
                    "aes_key": "6363636363636363636363636363636363636363636363636363636363636363", // 32 bytes of 'c'
                    "hmac_key": "6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a", // 16 bytes of 'j'
                    "expire": "2025-11-30T23:59:59Z"
                }
            ]
        });

        let result = RemoteKeys::parse_json(&json_value);
        assert!(result.is_ok());

        let remote_keys = result.unwrap();
        assert_eq!(remote_keys.dec.len(), 2);

        // Missing encrypt key
        let json_value = json!({
            "decrypt": [valid_decrypt_key_json()]
        });
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        // Invalid encrypt key
        let json_value = json!({
            "encrypt": "not an object",
            "decrypt": [valid_decrypt_key_json()]
        });
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        // Invalid decrypt key
        let json_value = json!({
            "encrypt": valid_encrypt_key_json(),
            "decrypt": "not an object"
        });
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        // Invalid key
        let json_value = json!({
            "unknown_field": "value"
        });
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        // Non-object JSON
        let json_value = json!("string");
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        let json_value = json!([]);
        assert!(RemoteKeys::parse_json(&json_value).is_err());

        let json_value = json!(true);
        assert!(RemoteKeys::parse_json(&json_value).is_err());
    }
}
