/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_geoip_types::{ContinentCode, IpLocation, IpLocationBuilder, IsoCountryCode};

pub fn as_iso_country_code(value: &Yaml) -> anyhow::Result<IsoCountryCode> {
    if let Yaml::String(s) = value {
        let country =
            IsoCountryCode::from_str(s).map_err(|_| anyhow!("invalid iso country code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'iso country code' should be 'string'"
        ))
    }
}

pub fn as_continent_code(value: &Yaml) -> anyhow::Result<ContinentCode> {
    if let Yaml::String(s) = value {
        let country = ContinentCode::from_str(s).map_err(|_| anyhow!("invalid continent code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'continent code' should be 'string'"
        ))
    }
}

pub fn as_ip_location(value: &Yaml) -> anyhow::Result<IpLocation> {
    if let Yaml::Hash(map) = value {
        let mut builder = IpLocationBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "network" | "net" => {
                let net = crate::value::as_ip_network(v)
                    .context(format!("invalid ip network value for key {k}"))?;
                builder.set_network(net);
                Ok(())
            }
            "country" => {
                let country = as_iso_country_code(v)
                    .context(format!("invalid iso country code value for key {k}"))?;
                builder.set_country(country);
                Ok(())
            }
            "continent" => {
                let continent = as_continent_code(v)
                    .context(format!("invalid continent code value for key {k}"))?;
                builder.set_continent(continent);
                Ok(())
            }
            "as_number" | "asn" => {
                let asn =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                builder.set_as_number(asn);
                Ok(())
            }
            "isp_name" => {
                let name = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                builder.set_isp_name(name);
                Ok(())
            }
            "isp_domain" => {
                let domain = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                builder.set_isp_domain(domain);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        builder.build()
    } else {
        Err(anyhow!("yaml value type for 'ip location' should be 'map'"))
    }
}

#[cfg(test)]
#[cfg(feature = "acl-rule")]
#[cfg(feature = "geoip")]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_iso_country_code_ok() {
        // valid country codes
        assert_eq!(
            as_iso_country_code(&yaml_str!("US")).unwrap(),
            IsoCountryCode::US
        );
        assert_eq!(
            as_iso_country_code(&yaml_str!("CN")).unwrap(),
            IsoCountryCode::CN
        );
        assert_eq!(
            as_iso_country_code(&yaml_str!("JP")).unwrap(),
            IsoCountryCode::JP
        );
    }

    #[test]
    fn as_iso_country_code_err() {
        // invalid string value
        assert!(as_iso_country_code(&yaml_str!("INVALID")).is_err());

        // non-string types
        assert!(as_iso_country_code(&Yaml::Integer(123)).is_err());
        assert!(as_iso_country_code(&Yaml::Boolean(true)).is_err());
        assert!(as_iso_country_code(&Yaml::Null).is_err());
    }

    #[test]
    fn as_continent_code_ok() {
        // valid continent codes
        assert_eq!(
            as_continent_code(&yaml_str!("AS")).unwrap(),
            ContinentCode::AS
        );
        assert_eq!(
            as_continent_code(&yaml_str!("EU")).unwrap(),
            ContinentCode::EU
        );
        assert_eq!(
            as_continent_code(&yaml_str!("NA")).unwrap(),
            ContinentCode::NA
        );
    }

    #[test]
    fn as_continent_code_err() {
        // invalid string value
        assert!(as_continent_code(&yaml_str!("INVALID")).is_err());

        // non-string types
        assert!(as_continent_code(&Yaml::Integer(123)).is_err());
        assert!(as_continent_code(&Yaml::Boolean(false)).is_err());
        assert!(as_continent_code(&Yaml::Null).is_err());
    }

    #[test]
    fn as_ip_location_ok() {
        // full valid location
        let yaml = yaml_doc!(
            r#"
                network: "192.168.0.0/24"
                country: "CN"
                continent: "AS"
                as_number: 1234
                isp_name: "Example ISP"
                isp_domain: "example.com"
            "#
        );

        let loc = as_ip_location(&yaml).unwrap();
        assert_eq!(loc.network_addr().to_string(), "192.168.0.0/24");
        assert_eq!(loc.country(), Some(IsoCountryCode::CN));
        assert_eq!(loc.continent(), Some(ContinentCode::AS));
        assert_eq!(loc.network_asn(), Some(1234));
        assert_eq!(loc.isp_name(), Some("Example ISP"));
        assert_eq!(loc.isp_domain(), Some("example.com"));

        // alias keys
        let yaml = yaml_doc!(
            r#"
                net: "192.168.0.0/24"
                asn: 1234
            "#
        );
        let loc = as_ip_location(&yaml).unwrap();
        assert_eq!(loc.network_addr().to_string(), "192.168.0.0/24");
        assert_eq!(loc.network_asn(), Some(1234));

        // minimum required fields (only network)
        let yaml = yaml_doc!(
            r#"
            network: "10.0.0.0/8"
            "#
        );
        assert_eq!(
            as_ip_location(&yaml).unwrap().network_addr().to_string(),
            "10.0.0.0/8"
        );
    }

    #[test]
    fn as_ip_location_err() {
        // non-hash type
        assert!(as_ip_location(&yaml_str!("invalid")).is_err());

        // missing network field
        let yaml_no_net = yaml_doc!(
            r#"
            country: "US"
            "#
        );
        assert!(as_ip_location(&yaml_no_net).is_err());

        // invalid country code
        let yaml_bad_country = yaml_doc!(
            r#"
            network: "192.168.0.0/24"
            country: "INVALID"
            "#
        );
        assert!(as_ip_location(&yaml_bad_country).is_err());

        // invalid continent code
        let yaml_bad_continent = yaml_doc!(
            r#"
            network: "192.168.0.0/24"
            continent: "INVALID"
            "#
        );
        assert!(as_ip_location(&yaml_bad_continent).is_err());

        // invalid as_number type
        let yaml_bad_asn = yaml_doc!(
            r#"
            network: "192.168.0.0/24"
            as_number: "not_a_number"
            "#
        );
        assert!(as_ip_location(&yaml_bad_asn).is_err());

        // invalid network format
        let yaml_bad_net = yaml_doc!(
            r#"
            network: "invalid_network"
            "#
        );
        assert!(as_ip_location(&yaml_bad_net).is_err());

        // invalid key
        let yaml_invalid_key = yaml_doc!(
            r#"
            network: "10.0.0.0/8"
            invalid_key: "value"
            "#
        );
        assert!(as_ip_location(&yaml_invalid_key).is_err());
    }
}
