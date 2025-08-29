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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_ip_country_record() {
        let record = GeoIpCountryRecord {
            country: IsoCountryCode::US,
            continent: ContinentCode::NA,
        };
        assert_eq!(record.country, IsoCountryCode::US);
        assert_eq!(record.continent, ContinentCode::NA);
    }

    #[test]
    fn geo_ip_asn_record() {
        let record = GeoIpAsnRecord {
            number: 1234,
            name: Some("Test ISP".to_string()),
            domain: Some("test.com".to_string()),
        };
        assert_eq!(record.number, 1234);
        assert_eq!(record.isp_name(), Some("Test ISP"));
        assert_eq!(record.isp_domain(), Some("test.com"));

        let record = GeoIpAsnRecord {
            number: 5678,
            name: None,
            domain: None,
        };
        assert_eq!(record.number, 5678);
        assert_eq!(record.isp_name(), None);
        assert_eq!(record.isp_domain(), None);
    }
}
