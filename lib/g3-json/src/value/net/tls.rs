/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Value) -> anyhow::Result<TlsVersion> {
    match value {
        Value::String(s) => TlsVersion::from_str(s),
        Value::Number(n) => {
            let Some(f) = n.as_f64() else {
                return Err(anyhow!("invalid f64 number value"));
            };
            TlsVersion::try_from(f)
        }
        _ => Err(anyhow!(
            "json value type for tls version should be 'string' or 'float'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_tls_version_ok() {
        // all valid string representations for each TLS version
        let valid_versions = [
            (
                TlsVersion::TLS1_0,
                vec!["1.0", "tls10", "tls1.0", "tls1_0", "TLS10", "TLS1.0"],
            ),
            (
                TlsVersion::TLS1_1,
                vec!["1.1", "tls11", "tls1.1", "tls1_1", "TLS11", "TLS1.1"],
            ),
            (
                TlsVersion::TLS1_2,
                vec!["1.2", "tls12", "tls1.2", "tls1_2", "TLS12", "TLS1.2"],
            ),
            (
                TlsVersion::TLS1_3,
                vec!["1.3", "tls13", "tls1.3", "tls1_3", "TLS13", "TLS1.3"],
            ),
        ];

        for (expected_version, strings) in valid_versions {
            for s in strings {
                let value = json!(s);
                let version = as_tls_version(&value).unwrap();
                assert_eq!(version, expected_version);
            }
        }

        // all valid float representations
        let valid_floats = [1.0, 1.1, 1.2, 1.3];
        for f in valid_floats {
            let value = json!(f);
            let version = as_tls_version(&value).unwrap();
            assert_eq!(version, TlsVersion::try_from(f).unwrap());
        }

        // valid integer representation (converted to float)
        let value = json!(1);
        let version = as_tls_version(&value).unwrap();
        assert_eq!(version, TlsVersion::TLS1_0);
    }

    #[test]
    fn as_tls_version_err() {
        // invalid string representations
        let invalid_strings = ["0.9", "ssl3.0", "", "tls1", "tls2.0"];
        for s in invalid_strings {
            let value = json!(s);
            assert!(as_tls_version(&value).is_err());
        }

        // invalid float values
        let invalid_floats = [0.9, 1.5, 2.0, -1.0];
        for f in invalid_floats {
            let value = json!(f);
            assert!(as_tls_version(&value).is_err());
        }

        // invalid integer value (converted to float 2.0)
        let value = json!(2);
        assert!(as_tls_version(&value).is_err());

        // invalid types
        assert!(as_tls_version(&json!(true)).is_err());
        assert!(as_tls_version(&json!([])).is_err());
        assert!(as_tls_version(&json!({})).is_err());
        assert!(as_tls_version(&json!(null)).is_err());
    }
}
