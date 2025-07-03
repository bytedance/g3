/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Yaml) -> anyhow::Result<TlsVersion> {
    match value {
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 value: {e}"))?;
            TlsVersion::try_from(f)
        }
        Yaml::String(s) => TlsVersion::from_str(s),
        _ => Err(anyhow!(
            "yaml value type for tls version should be 'string' or 'float'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_tls_version_ok() {
        // Valid float values
        let version = as_tls_version(&Yaml::Real("1.0".to_string())).unwrap();
        assert_eq!(version, TlsVersion::TLS1_0);
        let version = as_tls_version(&Yaml::Real("1.1".to_string())).unwrap();
        assert_eq!(version, TlsVersion::TLS1_1);
        let version = as_tls_version(&Yaml::Real("1.2".to_string())).unwrap();
        assert_eq!(version, TlsVersion::TLS1_2);
        let version = as_tls_version(&Yaml::Real("1.3".to_string())).unwrap();
        assert_eq!(version, TlsVersion::TLS1_3);

        // Valid string values (all variants)
        for s in ["1.0", "tls10", "tls1.0", "tls1_0", "TLS10", "TLS1.0"] {
            let version = as_tls_version(&Yaml::String(s.to_string())).unwrap();
            assert_eq!(version, TlsVersion::TLS1_0);
        }
        for s in ["1.1", "tls11", "tls1.1", "tls1_1", "TLS11", "TLS1.1"] {
            let version = as_tls_version(&Yaml::String(s.to_string())).unwrap();
            assert_eq!(version, TlsVersion::TLS1_1);
        }
        for s in ["1.2", "tls12", "tls1.2", "tls1_2", "TLS12", "TLS1.2"] {
            let version = as_tls_version(&Yaml::String(s.to_string())).unwrap();
            assert_eq!(version, TlsVersion::TLS1_2);
        }
        for s in ["1.3", "tls13", "tls1.3", "tls1_3", "TLS13", "TLS1.3"] {
            let version = as_tls_version(&Yaml::String(s.to_string())).unwrap();
            assert_eq!(version, TlsVersion::TLS1_3);
        }
    }

    #[test]
    fn as_tls_version_err() {
        // Invalid float values
        assert!(as_tls_version(&Yaml::Real("0.9".to_string())).is_err());
        assert!(as_tls_version(&Yaml::Real("2.0".to_string())).is_err());
        assert!(as_tls_version(&Yaml::Real("invalid".to_string())).is_err());

        // Invalid string values
        assert!(as_tls_version(&yaml_str!("tls0.9")).is_err());
        assert!(as_tls_version(&yaml_str!("ssl3.0")).is_err());
        assert!(as_tls_version(&yaml_str!("")).is_err());

        // Non-float/string types
        assert!(as_tls_version(&Yaml::Boolean(true)).is_err());
        assert!(as_tls_version(&Yaml::Integer(1)).is_err());
        assert!(as_tls_version(&Yaml::Array(vec![])).is_err());
    }
}
