/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::auth::FastHashedPassPhrase;
use g3_xcrypt::XCryptHash;

use super::{CONFIG_KEY_TYPE, PasswordToken};

const CONFIG_KEY_SALT: &str = "salt";

fn as_fast_hash(map: &yaml::Hash) -> anyhow::Result<FastHashedPassPhrase> {
    let salt = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SALT)?;
    let mut pass = FastHashedPassPhrase::new(salt)?;

    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        CONFIG_KEY_TYPE => Ok(()),
        CONFIG_KEY_SALT => Ok(()),
        "md5" => {
            if let Yaml::String(s) = v {
                pass.push_md5(s)
                    .context(format!("invalid md5 hash string value for key {k}"))
            } else {
                Err(anyhow!(
                    "yaml value type for 'md5 hash string' should be 'string'"
                ))
            }
        }
        "sha1" => {
            if let Yaml::String(s) = v {
                pass.push_sha1(s)
                    .context(format!("invalid sha1 hash string value for key {k}"))
            } else {
                Err(anyhow!(
                    "yaml value type for 'sha1 hash string' should be 'string'"
                ))
            }
        }
        "blake3" | "b3" => {
            if let Yaml::String(s) = v {
                pass.push_blake3(s)
                    .context(format!("invalid blake3 hash string value for key {k}"))
            } else {
                Err(anyhow!(
                    "yaml value type for 'blake3 hash string' should be 'string'"
                ))
            }
        }
        _ => Err(anyhow!("invalid key {}", k)),
    })?;
    pass.check_config()?;

    Ok(pass)
}

fn as_xcrypt_hash(v: &Yaml) -> anyhow::Result<XCryptHash> {
    match v {
        Yaml::String(s) => XCryptHash::parse(s).map_err(|e| anyhow!("invalid xcrypt string: {e}")),
        Yaml::Hash(map) => {
            let s = g3_yaml::hash_get_required_str(map, "value")?;
            XCryptHash::parse(s).map_err(|e| anyhow!("invalid xcrypt string: {e}"))
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

impl PasswordToken {
    pub(crate) fn parse_yaml(v: &Yaml) -> anyhow::Result<Self> {
        match v {
            Yaml::String(_) => Ok(PasswordToken::XCrypt(as_xcrypt_hash(v)?)),
            Yaml::Hash(map) => {
                if let Ok(map_type) = g3_yaml::hash_get_required_str(map, CONFIG_KEY_TYPE) {
                    match g3_yaml::key::normalize(map_type).as_str() {
                        "fast_hash" => Ok(PasswordToken::FastHash(as_fast_hash(map)?)),
                        "xcrypt_hash" => Ok(PasswordToken::XCrypt(as_xcrypt_hash(v)?)),
                        _ => Err(anyhow!("unsupported user authentication type")),
                    }
                } else {
                    Ok(PasswordToken::FastHash(as_fast_hash(map)?))
                }
            }
            Yaml::Null => Ok(PasswordToken::SkipVerify),
            _ => Err(anyhow!("invalid value type")),
        }
    }
}
