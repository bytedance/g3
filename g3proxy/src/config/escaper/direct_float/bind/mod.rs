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

use std::net::IpAddr;

use ahash::AHashMap;
use chrono::{DateTime, Utc};
use rand::seq::IteratorRandom;
use tokio::time::Instant;

use g3_socket::util::AddressFamily;
use g3_types::net::EgressInfo;

mod json;

const CONFIG_KEY_IP: &str = "ip";
const CONFIG_KEY_ID: &str = "id";
const CONFIG_KEY_EXPIRE: &str = "expire";
const CONFIG_KEY_ISP: &str = "isp";
const CONFIG_KEY_EIP: &str = "eip";
const CONFIG_KEY_AREA: &str = "area";

#[derive(Clone, Debug)]
pub(crate) struct DirectFloatBindIp {
    id: Option<String>,
    pub(crate) ip: IpAddr,
    pub(crate) expire_datetime: Option<DateTime<Utc>>,
    expire_instant: Option<Instant>,
    pub(crate) egress_info: EgressInfo,
}

impl DirectFloatBindIp {
    fn new(ip: IpAddr) -> Self {
        DirectFloatBindIp {
            id: None,
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

    pub(crate) fn is_expired(&self) -> bool {
        if let Some(expire) = self.expire_instant {
            expire.checked_duration_since(Instant::now()).is_none()
        } else {
            false
        }
    }

    pub(crate) fn expected_alive_minutes(&self) -> u64 {
        if let Some(expire) = self.expire_instant {
            expire
                .checked_duration_since(Instant::now())
                .map(|d| d.as_secs() / 60)
                .unwrap_or(0)
        } else {
            u64::MAX
        }
    }
}

pub(crate) struct BindSet {
    family: AddressFamily,
    unnamed: Vec<DirectFloatBindIp>,
    named: AHashMap<String, DirectFloatBindIp>,
}

impl BindSet {
    pub(crate) fn new(family: AddressFamily) -> Self {
        BindSet {
            family,
            unnamed: Vec::with_capacity(4),
            named: AHashMap::with_capacity(4),
        }
    }

    fn push(&mut self, mut bind: DirectFloatBindIp) {
        if AddressFamily::from(&bind.ip).ne(&self.family) {
            return;
        }
        if let Some(id) = bind.id.take() {
            self.named.insert(id, bind);
        } else {
            self.unnamed.push(bind);
        }
    }

    pub(crate) fn select_random_bind(&self) -> Option<DirectFloatBindIp> {
        self.unnamed
            .iter()
            .chain(self.named.values())
            .choose(&mut rand::thread_rng())
            .cloned()
    }

    pub(crate) fn select_again(&self, ip: IpAddr) -> Option<DirectFloatBindIp> {
        self.unnamed
            .iter()
            .chain(self.named.values())
            .find(|v| v.ip == ip)
            .cloned()
    }

    pub(crate) fn select_stable_bind(&self) -> Option<&DirectFloatBindIp> {
        if self.unnamed.len() == 1 {
            return self.unnamed.first();
        }
        if self.named.len() == 1 {
            return self.named.values().next();
        }
        None
    }

    #[inline]
    pub(crate) fn select_named_bind(&self, id: &str) -> Option<DirectFloatBindIp> {
        self.named.get(id).cloned()
    }
}
