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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn egress_area_from_str() {
        let area = EgressArea::from_str("region").unwrap();
        assert_eq!(area.to_string(), "region");

        let area = EgressArea::from_str("asia/china/beijing").unwrap();
        assert_eq!(area.to_string(), "asia/china/beijing");

        let area = EgressArea::from_str("/region/country/").unwrap();
        assert_eq!(area.to_string(), "region/country");

        let area = EgressArea::from_str("region//country///city").unwrap();
        assert_eq!(area.to_string(), "region/country/city");

        let area = EgressArea::from_str("  region/country  ").unwrap();
        assert_eq!(area.to_string(), "region/country");

        let result = EgressArea::from_str("");
        assert!(result.is_err());

        let result = EgressArea::from_str("///");
        assert!(result.is_err());

        let result = EgressArea::from_str("   ");
        assert!(result.is_err());
    }

    #[test]
    fn egress_info_operations() {
        let mut info = EgressInfo::default();

        let ipv4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        info.set_ip(ipv4);
        assert_eq!(info.ip(), Some(ipv4));

        let ipv6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        info.set_ip(ipv6);
        assert_eq!(info.ip(), Some(ipv6));

        info.set_isp("China Telecom".to_string());
        assert_eq!(info.isp(), Some("China Telecom"));

        info.set_isp("China Mobile".to_string());
        assert_eq!(info.isp(), Some("China Mobile"));

        let area = EgressArea::from_str("region/country/city").unwrap();
        info.set_area(area.clone());
        assert_eq!(info.area().unwrap().to_string(), area.to_string());

        info.reset();
        assert!(info.ip().is_none());
        assert!(info.isp().is_none());
        assert!(info.area().is_none());
    }
}
