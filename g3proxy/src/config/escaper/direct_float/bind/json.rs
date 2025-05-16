/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::Instant;

use g3_socket::util::AddressFamily;

use super::{
    BindSet, CONFIG_KEY_AREA, CONFIG_KEY_EIP, CONFIG_KEY_EXPIRE, CONFIG_KEY_ID, CONFIG_KEY_IP,
    CONFIG_KEY_ISP, DirectFloatBindIp,
};

impl DirectFloatBindIp {
    pub(crate) fn parse_json(
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
                        CONFIG_KEY_ID => {
                            let id = g3_json::value::as_string(v)?;
                            bind.id = Some(id);
                        }
                        CONFIG_KEY_EXPIRE => {
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
                        CONFIG_KEY_ISP => {
                            if let Ok(isp) = g3_json::value::as_string(v) {
                                bind.egress_info.set_isp(isp);
                            }
                            // not a required field, skip if value format is invalid
                        }
                        CONFIG_KEY_EIP => {
                            if let Ok(ip) = g3_json::value::as_ipaddr(v) {
                                bind.egress_info.set_ip(ip);
                            }
                            // not a required field, skip if value format is invalid
                        }
                        CONFIG_KEY_AREA => {
                            if let Ok(area) = g3_json::value::as_egress_area(v) {
                                bind.egress_info.set_area(area);
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

impl BindSet {
    pub(crate) fn parse_json(value: &Value, family: AddressFamily) -> anyhow::Result<Self> {
        let mut bind_set = BindSet::new(family);

        let instant_now = Instant::now();
        let datetime_now = Utc::now();

        match value {
            Value::Null => {}
            Value::Array(records) => {
                for (i, record) in records.iter().enumerate() {
                    if let Some(bind) =
                        DirectFloatBindIp::parse_json(record, instant_now, datetime_now)
                            .context(format!("invalid value for record #{i}"))?
                    {
                        bind_set.push(bind);
                    };
                }
            }
            _ => {
                if let Some(bind) = DirectFloatBindIp::parse_json(value, instant_now, datetime_now)
                    .context("invalid single bind ip value")?
                {
                    bind_set.push(bind);
                }
            }
        }

        Ok(bind_set)
    }
}
