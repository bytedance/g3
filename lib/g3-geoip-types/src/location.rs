/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use ip_network::IpNetwork;
use smol_str::SmolStr;

use super::{ContinentCode, IsoCountryCode};

#[derive(Default)]
pub struct IpLocationBuilder {
    net: Option<IpNetwork>,
    country: Option<IsoCountryCode>,
    continent: Option<ContinentCode>,
    as_number: Option<u32>,
    isp_name: Option<SmolStr>,
    isp_domain: Option<SmolStr>,
}

impl IpLocationBuilder {
    pub fn set_network(&mut self, net: IpNetwork) {
        if let Some(old_net) = self.net.take() {
            if old_net.netmask() < net.netmask() {
                self.net = Some(net);
            } else {
                self.net = Some(old_net);
            }
        } else {
            self.net = Some(net);
        }
    }

    pub fn set_country(&mut self, country: IsoCountryCode) {
        self.country = Some(country);
    }

    pub fn set_continent(&mut self, continent: ContinentCode) {
        self.continent = Some(continent);
    }

    pub fn set_as_number(&mut self, number: u32) {
        self.as_number = Some(number);
    }

    pub fn set_isp_name(&mut self, name: String) {
        self.isp_name = Some(name.into());
    }

    pub fn set_isp_domain(&mut self, domain: String) {
        self.isp_domain = Some(domain.into());
    }

    pub fn build(mut self) -> anyhow::Result<IpLocation> {
        let net = self
            .net
            .take()
            .ok_or(anyhow!("network address is not set"))?;
        let continent = self
            .continent
            .or_else(|| self.country.map(|c| c.continent()));
        Ok(IpLocation {
            net,
            country: self.country,
            continent,
            as_number: self.as_number,
            isp_name: self.isp_name,
            isp_domain: self.isp_domain,
        })
    }
}

pub struct IpLocation {
    net: IpNetwork,
    country: Option<IsoCountryCode>,
    continent: Option<ContinentCode>,
    as_number: Option<u32>,
    isp_name: Option<SmolStr>,
    isp_domain: Option<SmolStr>,
}

impl IpLocation {
    #[inline]
    pub fn network_addr(&self) -> IpNetwork {
        self.net
    }

    #[inline]
    pub fn country(&self) -> Option<IsoCountryCode> {
        self.country
    }

    #[inline]
    pub fn continent(&self) -> Option<ContinentCode> {
        self.continent
    }

    #[inline]
    pub fn network_asn(&self) -> Option<u32> {
        self.as_number
    }

    #[inline]
    pub fn isp_name(&self) -> Option<&str> {
        self.isp_name.as_deref()
    }

    #[inline]
    pub fn isp_domain(&self) -> Option<&str> {
        self.isp_domain.as_deref()
    }
}
