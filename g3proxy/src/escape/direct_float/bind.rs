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

use std::net::IpAddr;

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::Instant;

use g3_socket::util::AddressFamily;
use g3_types::net::EgressInfo;

const CONFIG_KEY_IP: &str = "ip";

#[derive(Clone, Debug)]
pub(super) struct DirectFloatBindIp {
    pub(super) ip: IpAddr,
    pub(super) expire_datetime: Option<DateTime<Utc>>,
    expire_instant: Option<Instant>,
    pub(super) egress_info: EgressInfo,
}

impl DirectFloatBindIp {
    fn new(ip: IpAddr) -> Self {
        DirectFloatBindIp {
            ip,
            expire_datetime: None,
            expire_instant: None,
            egress_info: Default::default(),
        }
    }

    fn set_expire(&mut self, datetime: DateTime<Utc>, instant: Instant) {
        self.expire_datetime = Some(datetime);
        self.expire_instant = Some(instant);
    }

    pub(super) fn is_expired(&self) -> bool {
        if let Some(expire) = self.expire_instant {
            expire.checked_duration_since(Instant::now()).is_none()
        } else {
            false
        }
    }

    pub(super) fn expected_alive_minutes(&self) -> u64 {
        if let Some(expire) = self.expire_instant {
            expire
                .checked_duration_since(Instant::now())
                .map(|d| d.as_secs() / 60)
                .unwrap_or(0)
        } else {
            u64::MAX
        }
    }

    fn parse_json(
        value: &Value,
        instant_now: Instant,
        datetime_now: DateTime<Utc>,
    ) -> anyhow::Result<Option<Self>> {
        match value {
            Value::Object(map) => {
                let ip_v = g3_json::map_get_required(map, CONFIG_KEY_IP)?;
                let ip = g3_json::value::as_ipaddr(ip_v)
                    .context(format!("invalid value for key {CONFIG_KEY_IP}"))?;

                let mut bind = DirectFloatBindIp::new(ip);

                for (k, v) in map {
                    match g3_json::key::normalize(k).as_str() {
                        CONFIG_KEY_IP => {}
                        "expire" => {
                            let datetime_expire = g3_json::value::as_rfc3339_datetime(v)?;
                            if datetime_expire < datetime_now {
                                return Ok(None);
                            }
                            let Ok(duration) =
                                datetime_expire.signed_duration_since(datetime_now).to_std()
                            else {
                                return Ok(None);
                            };
                            let Some(instant_expire) = instant_now.checked_add(duration) else {
                                return Ok(None);
                            };
                            bind.set_expire(datetime_expire, instant_expire);
                        }
                        "isp" => {
                            if let Ok(isp) = g3_json::value::as_string(v) {
                                bind.egress_info.isp = Some(isp);
                            }
                            // not a required field, skip if value format is invalid
                        }
                        "eip" => {
                            if let Ok(ip) = g3_json::value::as_ipaddr(v) {
                                bind.egress_info.ip = Some(ip);
                            }
                            // not a required field, skip if value format is invalid
                        }
                        "area" => {
                            if let Ok(area) = g3_json::value::as_egress_area(v) {
                                bind.egress_info.area = Some(area);
                            }
                            // not a required field, skip if value format is invalid
                        }
                        _ => return Err(anyhow!("invalid key {}", k)),
                    }
                }

                Ok(Some(bind))
            }
            Value::String(_) => {
                let ip = g3_json::value::as_ipaddr(value)
                    .context(anyhow!("invalid ip address value"))?;
                Ok(Some(DirectFloatBindIp::new(ip)))
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub(super) fn parse_records(
    records: &[Value],
    family: AddressFamily,
) -> anyhow::Result<Vec<DirectFloatBindIp>> {
    let mut ips = Vec::<DirectFloatBindIp>::new();

    let instant_now = Instant::now();
    let datetime_now = Utc::now();

    for (i, record) in records.iter().enumerate() {
        let Some(bind) = DirectFloatBindIp::parse_json(record, instant_now, datetime_now)
            .context(format!("invalid value for record #{i}"))?
        else {
            continue;
        };

        if AddressFamily::from(&bind.ip).ne(&family) {
            continue;
        }

        ips.push(bind);
    }

    Ok(ips)
}
