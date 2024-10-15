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
use redis::AsyncCommands;

use g3_redis_client::{RedisClientConfig, RedisClientConfigBuilder};

use super::{RemoteDecryptKey, RemoteEncryptKey, RemoteKeys};

#[cfg(feature = "yaml")]
mod yaml;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RedisSourceConfig {
    redis: RedisClientConfigBuilder,
    enc_key_name: String,
    dec_set_name: String,
}

impl RedisSourceConfig {
    pub(super) fn build(&self) -> anyhow::Result<RedisSource> {
        let redis = self.redis.build()?;
        Ok(RedisSource {
            redis,
            enc_key_name: self.enc_key_name.clone(),
            dec_set_name: self.dec_set_name.clone(),
        })
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.enc_key_name.is_empty() {
            return Err(anyhow!("no enc redis keys name set"));
        }
        if self.dec_set_name.is_empty() {
            return Err(anyhow!("no dec redis set name set"));
        }
        Ok(())
    }
}

pub(crate) struct RedisSource {
    redis: RedisClientConfig,
    enc_key_name: String,
    dec_set_name: String,
}

impl RedisSource {
    pub(crate) async fn fetch_remote_keys(&self) -> anyhow::Result<RemoteKeys> {
        let mut conn = self
            .redis
            .connect()
            .await
            .context("failed to connect to redis")?;

        let enc_key = conn
            .get(&self.enc_key_name)
            .await
            .map_err(|e| anyhow!("failed to get redis key {}: {e}", self.enc_key_name))?;
        let redis::Value::BulkString(b) = enc_key else {
            return Err(anyhow!(
                "invalid data type for redis key {}",
                self.enc_key_name
            ));
        };
        let record = serde_json::from_slice(&b).map_err(|e| {
            anyhow!(
                "invalid json string in redis key {}: {e}",
                self.enc_key_name
            )
        })?;
        let enc_key = RemoteEncryptKey::parse_json(&record).context("invalid encrypt key")?;

        let members: Vec<redis::Value> = conn.smembers(&self.dec_set_name).await.map_err(|e| {
            anyhow!(
                "failed to get all members of sets {}: {e}",
                self.dec_set_name
            )
        })?;
        let mut dec_keys = Vec::with_capacity(members.len());
        for (i, m) in members.into_iter().enumerate() {
            let redis::Value::BulkString(b) = m else {
                return Err(anyhow!(
                    "invalid data type for redis set value {}#{i}",
                    self.dec_set_name
                ));
            };
            let record = serde_json::from_slice(&b).map_err(|e| {
                anyhow!(
                    "invalid json string in redis set value {}#{i}: {e}",
                    self.dec_set_name
                )
            })?;
            let dec_key = RemoteDecryptKey::parse_json(&record).context("invalid decrypt key")?;
            dec_keys.push(dec_key);
        }

        Ok(RemoteKeys {
            enc: enc_key,
            dec: dec_keys,
        })
    }
}
