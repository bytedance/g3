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
use serde_json::{Map, Value};

use g3_types::auth::FastHashedPassPhrase;
use g3_xcrypt::XCryptHash;

use super::{UserAuthentication, CONFIG_KEY_TYPE};

const CONFIG_KEY_SALT: &str = "salt";

fn as_fast_hash(map: &Map<String, Value>) -> anyhow::Result<FastHashedPassPhrase> {
    let salt = g3_json::get_required_str(map, CONFIG_KEY_SALT)?;
    let mut pass = FastHashedPassPhrase::new(salt)?;

    for (k, v) in map {
        match g3_json::key::normalize(k).as_str() {
            CONFIG_KEY_TYPE => {}
            CONFIG_KEY_SALT => {}
            "md5" => {
                if let Value::String(s) = v {
                    pass.push_md5(s)
                        .context(format!("invalid md5 hash string value for key {k}"))?;
                } else {
                    return Err(anyhow!(
                        "json value type for 'md5 hash string' should be 'string'"
                    ));
                }
            }
            "sha1" => {
                if let Value::String(s) = v {
                    pass.push_sha1(s)
                        .context(format!("invalid sha1 hash string value for key {k}"))?;
                } else {
                    return Err(anyhow!(
                        "json value type for 'sha1 hash string' should be 'string'"
                    ));
                }
            }
            "blake3" | "b3" => {
                if let Value::String(s) = v {
                    pass.push_blake3(s)
                        .context(format!("invalid blake3 hash string value for key {k}"))?;
                } else {
                    return Err(anyhow!(
                        "json value type for 'blake3 hash string' should be 'string'"
                    ));
                }
            }
            _ => return Err(anyhow!("invalid key {k}")),
        }
    }
    pass.check_config()?;

    Ok(pass)
}

fn as_xcrypt_hash(v: &Value) -> anyhow::Result<XCryptHash> {
    match v {
        Value::String(s) => XCryptHash::parse(s).map_err(|e| anyhow!("invalid xcrypt string: {e}")),
        Value::Object(map) => {
            let s = g3_json::get_required_str(map, "value")?;
            XCryptHash::parse(s).map_err(|e| anyhow!("invalid xcrypt string: {e}"))
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

impl UserAuthentication {
    pub(crate) fn parse_json(v: &Value) -> anyhow::Result<Self> {
        match v {
            Value::String(_) => Ok(UserAuthentication::XCrypt(as_xcrypt_hash(v)?)),
            Value::Object(map) => {
                if let Ok(map_type) = g3_json::get_required_str(map, CONFIG_KEY_TYPE) {
                    match g3_json::key::normalize(map_type).as_str() {
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
