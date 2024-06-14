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

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct EgressArea {
    inner: Vec<Arc<str>>,
}

impl fmt::Display for EgressArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner.join("/"))
    }
}

impl FromStr for EgressArea {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw: Vec<_> = s.trim().split('/').collect();
        let mut inner = Vec::with_capacity(raw.len());
        for s in raw {
            if s.is_empty() {
                continue;
            }
            inner.push(Arc::from(s));
        }
        if inner.is_empty() {
            return Err(());
        }
        Ok(EgressArea { inner })
    }
}

#[derive(Clone, Debug, Default)]
pub struct EgressInfo {
    ip: Option<IpAddr>,
    isp: Option<Arc<str>>,
    area: Option<EgressArea>,
}

impl EgressInfo {
    pub fn reset(&mut self) {
        self.ip = None;
        self.isp = None;
        self.area = None;
    }

    #[inline]
    pub fn ip(&self) -> Option<IpAddr> {
        self.ip
    }

    pub fn set_ip(&mut self, ip: IpAddr) {
        self.ip = Some(ip);
    }

    #[inline]
    pub fn isp(&self) -> Option<&str> {
        self.isp.as_deref()
    }

    pub fn set_isp(&mut self, isp: String) {
        self.isp = Some(Arc::from(isp));
    }

    #[inline]
    pub fn area(&self) -> Option<&EgressArea> {
        self.area.as_ref()
    }

    pub fn set_area(&mut self, area: EgressArea) {
        self.area = Some(area);
    }
}
