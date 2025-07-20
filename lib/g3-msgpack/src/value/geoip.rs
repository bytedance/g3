/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use rmpv::ValueRef;

use g3_geoip_types::{ContinentCode, IpLocation, IpLocationBuilder, IsoCountryCode};

pub fn as_iso_country_code(value: &ValueRef) -> anyhow::Result<IsoCountryCode> {
    let s = crate::value::as_string(value)
        .context("msgpack 'string' value type is expected for iso country code")?;
    let country = IsoCountryCode::from_str(&s).map_err(|_| anyhow!("invalid iso country code"))?;
    Ok(country)
}

pub fn as_continent_code(value: &ValueRef) -> anyhow::Result<ContinentCode> {
    let s = crate::value::as_string(value)
        .context("msgpack 'string' value type is expected for continent code")?;
    let country = ContinentCode::from_str(&s).map_err(|_| anyhow!("invalid continent code"))?;
    Ok(country)
}

pub fn as_ip_location(value: &ValueRef) -> anyhow::Result<IpLocation> {
    if let ValueRef::Map(map) = value {
        let mut builder = IpLocationBuilder::default();

        for (k, v) in map {
            let k =
                crate::value::as_string(k).context("key of the map is not a valid string value")?;
            match crate::key::normalize(&k).as_str() {
                "network" | "net" => {
                    let net = crate::value::as_ip_network(v)
                        .context(format!("invalid ip network value for key {k}"))?;
                    builder.set_network(net);
                }
                "country" => {
                    let country = as_iso_country_code(v)
                        .context(format!("invalid iso country code value for key {k}"))?;
                    builder.set_country(country);
                }
                "continent" => {
                    let continent = as_continent_code(v)
                        .context(format!("invalid continent code value for key {k}"))?;
                    builder.set_continent(continent);
                }
                "as_number" | "asn" => {
                    let asn = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    builder.set_as_number(asn);
                }
                "isp_name" => {
                    let name = crate::value::as_string(v)
                        .context(format!("invalid string value for key {k}"))?;
                    builder.set_isp_name(name);
                }
                "isp_domain" => {
                    let domain = crate::value::as_string(v)
                        .context(format!("invalid string value for key {k}"))?;
                    builder.set_isp_domain(domain);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        builder.build()
    } else {
        Err(anyhow!(
            "msgpack value type for 'ip location' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_iso_country_code_ok() {
        // valid alpha2 codes
        let us = ValueRef::from("US");
        assert_eq!(as_iso_country_code(&us).unwrap(), IsoCountryCode::US);

        let cn = ValueRef::from("CN");
        assert_eq!(as_iso_country_code(&cn).unwrap(), IsoCountryCode::CN);

        // valid alpha3 codes
        let usa = ValueRef::from("USA");
        assert_eq!(as_iso_country_code(&usa).unwrap(), IsoCountryCode::US);

        let chn = ValueRef::from("CHN");
        assert_eq!(as_iso_country_code(&chn).unwrap(), IsoCountryCode::CN);
    }

    #[test]
    fn as_iso_country_code_err() {
        // invalid country code
        let invalid = ValueRef::from("INVALID");
        assert!(as_iso_country_code(&invalid).is_err());
    }

    #[test]
    fn as_continent_code_ok() {
        // valid continent codes
        let asia = ValueRef::from("AS");
        assert_eq!(as_continent_code(&asia).unwrap(), ContinentCode::AS);

        let europe = ValueRef::from("EU");
        assert_eq!(as_continent_code(&europe).unwrap(), ContinentCode::EU);

        let north_america = ValueRef::from("NA");
        assert_eq!(
            as_continent_code(&north_america).unwrap(),
            ContinentCode::NA
        );
    }

    #[test]
    fn as_continent_code_err() {
        // invalid continent code
        let invalid = ValueRef::from("XX");
        assert!(as_continent_code(&invalid).is_err());

        // wrong value type
        let number = ValueRef::from(42);
        assert!(as_continent_code(&number).is_err());

        // empty string
        let empty = ValueRef::from("");
        assert!(as_continent_code(&empty).is_err());
    }

    #[test]
    fn as_ip_location_ok() {
        // minimal valid location (only network)
        let minimal = vec![(ValueRef::from("network"), ValueRef::from("192.168.0.0/24"))];
        let minimal_value = ValueRef::Map(minimal);
        let minimal_loc = as_ip_location(&minimal_value).unwrap();
        assert_eq!(minimal_loc.network_addr().to_string(), "192.168.0.0/24");
        assert!(minimal_loc.country().is_none());

        // full location
        let full = vec![
            (ValueRef::from("network"), ValueRef::from("10.0.0.0/8")),
            (ValueRef::from("country"), ValueRef::from("US")),
            (ValueRef::from("continent"), ValueRef::from("NA")),
            (ValueRef::from("as_number"), ValueRef::from(12345)),
            (ValueRef::from("isp_name"), ValueRef::from("Test ISP")),
            (ValueRef::from("isp_domain"), ValueRef::from("test.com")),
        ];
        let full_value = ValueRef::Map(full);
        let full_loc = as_ip_location(&full_value).unwrap();
        assert_eq!(full_loc.network_addr().to_string(), "10.0.0.0/8");
        assert_eq!(full_loc.country(), Some(IsoCountryCode::US));
        assert_eq!(full_loc.continent(), Some(ContinentCode::NA));
        assert_eq!(full_loc.network_asn(), Some(12345));
        assert_eq!(full_loc.isp_name(), Some("Test ISP"));
        assert_eq!(full_loc.isp_domain(), Some("test.com"));

        // case insensitivity in keys
        let case_insensitive = vec![
            (ValueRef::from("NeTwOrK"), ValueRef::from("172.16.0.0/12")),
            (ValueRef::from("cOuNtRy"), ValueRef::from("CN")),
        ];
        let case_value = ValueRef::Map(case_insensitive);
        let case_loc = as_ip_location(&case_value).unwrap();
        assert_eq!(case_loc.network_addr().to_string(), "172.16.0.0/12");
        assert_eq!(case_loc.country(), Some(IsoCountryCode::CN));
    }

    #[test]
    fn as_ip_location_err() {
        // non-map value
        let array = ValueRef::Array(vec![]);
        assert!(as_ip_location(&array).is_err());

        // missing network field
        let no_network = vec![(ValueRef::from("country"), ValueRef::from("US"))];
        let no_network_value = ValueRef::Map(no_network);
        assert!(as_ip_location(&no_network_value).is_err());

        // invalid network format
        let invalid_network = vec![(ValueRef::from("network"), ValueRef::from("invalid"))];
        let invalid_network_value = ValueRef::Map(invalid_network);
        assert!(as_ip_location(&invalid_network_value).is_err());

        // invalid country code
        let invalid_country = vec![
            (ValueRef::from("network"), ValueRef::from("192.168.0.0/24")),
            (ValueRef::from("country"), ValueRef::from("INVALID")),
        ];
        let invalid_country_value = ValueRef::Map(invalid_country);
        assert!(as_ip_location(&invalid_country_value).is_err());

        // invalid continent code
        let invalid_continent = vec![
            (ValueRef::from("network"), ValueRef::from("192.168.0.0/24")),
            (ValueRef::from("continent"), ValueRef::from("XX")),
        ];
        let invalid_continent_value = ValueRef::Map(invalid_continent);
        assert!(as_ip_location(&invalid_continent_value).is_err());

        // invalid AS number type
        let invalid_asn = vec![
            (ValueRef::from("network"), ValueRef::from("192.168.0.0/24")),
            (ValueRef::from("asn"), ValueRef::from("invalid")),
        ];
        let invalid_asn_value = ValueRef::Map(invalid_asn);
        assert!(as_ip_location(&invalid_asn_value).is_err());

        // invalid key
        let invalid_key = vec![
            (ValueRef::from("network"), ValueRef::from("192.168.0.0/24")),
            (ValueRef::from("invalid_key"), ValueRef::from("value")),
        ];
        let invalid_key_value = ValueRef::Map(invalid_key);
        assert!(as_ip_location(&invalid_key_value).is_err());
    }
}
