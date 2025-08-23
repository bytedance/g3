/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_geoip_types::{ContinentCode, IsoCountryCode};

pub struct GeoIpCountryRecord {
    pub country: IsoCountryCode,
    pub continent: ContinentCode,
}

pub struct GeoIpAsnRecord {
    pub number: u32,
    pub(crate) name: Option<String>,
    pub(crate) domain: Option<String>,
}

impl GeoIpAsnRecord {
    pub fn isp_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn isp_domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
}
