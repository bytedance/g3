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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_types::auth::FastHashedPassPhrase;
use g3_xcrypt::XCryptHash;

use super::{UserAuthentication, CONFIG_KEY_TYPE};

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

impl UserAuthentication {
    pub(crate) fn parse_yaml(v: &Yaml) -> anyhow::Result<Self> {
        match v {
            Yaml::String(_) => Ok(UserAuthentication::XCrypt(as_xcrypt_hash(v)?)),
            Yaml::Hash(map) => {
                if let Ok(map_type) = g3_yaml::hash_get_required_str(map, CONFIG_KEY_TYPE) {
                    match g3_yaml::key::normalize(map_type).as_str() {
                        "fast_hash" => Ok(UserAuthentication::FastHash(as_fast_hash(map)?)),
                        "xcrypt_hash" => Ok(UserAuthentication::XCrypt(as_xcrypt_hash(v)?)),
                        _ => Err(anyhow!("unsupported user authentication type")),
                    }
                } else {
                    Ok(UserAuthentication::FastHash(as_fast_hash(map)?))
                }
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}
