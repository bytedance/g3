/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
