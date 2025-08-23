/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rmpv::ValueRef;

fn as_certificates_from_single_element(value: &ValueRef) -> anyhow::Result<Vec<X509>> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type 'certificates' should be 'string' or 'binary'"
            ));
        }
    };

    let certs =
        X509::stack_from_pem(bytes).map_err(|e| anyhow!("invalid certificate string: {e}"))?;
    if certs.is_empty() {
        Err(anyhow!("no valid certificate found"))
    } else {
        Ok(certs)
    }
}

pub fn as_openssl_certificates(value: &ValueRef) -> anyhow::Result<Vec<X509>> {
    if let ValueRef::Array(seq) = value {
        let mut certs = Vec::new();
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.extend(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value)
    }
}

pub fn as_openssl_certificate(value: &ValueRef) -> anyhow::Result<X509> {
    match value {
        ValueRef::String(s) => {
            X509::from_pem(s.as_bytes()).map_err(|e| anyhow!("invalid PEM encoded x509 cert: {e}"))
        }
        ValueRef::Binary(b) => {
            X509::from_der(b).map_err(|e| anyhow!("invalid DER encoded x509 cert: {e}"))
        }
        _ => Err(anyhow!(
            "msgpack value for 'certificate' should be 'pem string' or 'der binary'"
        )),
    }
}

pub fn as_openssl_private_key(value: &ValueRef) -> anyhow::Result<PKey<Private>> {
    match value {
        ValueRef::String(s) => PKey::private_key_from_pem(s.as_bytes())
            .map_err(|e| anyhow!("invalid PEM encoded private key: {e}")),
        ValueRef::Binary(b) => PKey::private_key_from_der(b)
            .map_err(|e| anyhow!("invalid DER encoded private key: {e}")),
        _ => Err(anyhow!(
            "msgpack value type for 'private key' should be 'pem string' or 'der binary'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CERT_PEM: &str = include_str!("test_data/test_cert.pem");
    const TEST_KEY_PEM: &str = include_str!("test_data/test_key.pem");

    #[test]
    fn as_openssl_certificates_ok() {
        // Single PEM string
        let value = ValueRef::String(TEST_CERT_PEM.into());
        let certs = as_openssl_certificates(&value).unwrap();
        assert!(!certs.is_empty());

        // Binary format
        let value = ValueRef::Binary(TEST_CERT_PEM.as_bytes());
        let certs = as_openssl_certificates(&value).unwrap();
        assert!(!certs.is_empty());

        // Array format
        let array = vec![
            ValueRef::String(TEST_CERT_PEM.into()),
            ValueRef::Binary(TEST_CERT_PEM.as_bytes()),
        ];
        let value = ValueRef::Array(array);
        let certs = as_openssl_certificates(&value).unwrap();
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn as_openssl_certificates_err() {
        // Invalid type
        let value = ValueRef::Integer(123.into());
        let result = as_openssl_certificates(&value);
        assert!(result.is_err());

        // Empty certificate
        let value = ValueRef::String("".into());
        let result = as_openssl_certificates(&value);
        assert!(result.is_err());

        // Invalid PEM format
        let value = ValueRef::String("invalid data".into());
        let result = as_openssl_certificates(&value);
        assert!(result.is_err());

        // Empty binary data
        let value = ValueRef::Binary(&[]);
        let result = as_openssl_certificates(&value);
        assert!(result.is_err());

        // Array element error
        let array = vec![
            ValueRef::String(TEST_CERT_PEM.into()), // valid
            ValueRef::Integer(456.into()),          // invalid
        ];
        let value = ValueRef::Array(array);
        let result = as_openssl_certificates(&value);
        assert!(result.is_err());
    }

    #[test]
    fn as_openssl_certificate_ok() {
        // PEM format
        let value = ValueRef::String(TEST_CERT_PEM.into());
        let cert = as_openssl_certificate(&value).unwrap();
        assert_eq!(cert.subject_name().entries().count(), 1);

        // DER format
        let der = X509::from_pem(TEST_CERT_PEM.as_bytes())
            .unwrap()
            .to_der()
            .unwrap();
        let value = ValueRef::Binary(&der);
        let cert = as_openssl_certificate(&value).unwrap();
        assert_eq!(cert.subject_name().entries().count(), 1);
    }

    #[test]
    fn as_openssl_certificate_err() {
        // Invalid type
        let value = ValueRef::Boolean(true);
        let result = as_openssl_certificate(&value);
        assert!(result.is_err());

        // Invalid PEM
        let value = ValueRef::String("invalid pem".into());
        let result = as_openssl_certificate(&value);
        assert!(result.is_err());

        // Invalid DER
        let value = ValueRef::Binary(&[0, 1, 2, 3]); // random invalid DER
        let result = as_openssl_certificate(&value);
        assert!(result.is_err());

        // Empty binary data
        let value = ValueRef::Binary(&[]);
        let result = as_openssl_certificate(&value);
        assert!(result.is_err());
    }

    #[test]
    fn as_openssl_private_key_ok() {
        // PEM format
        let value = ValueRef::String(TEST_KEY_PEM.into());
        let key = as_openssl_private_key(&value).unwrap();
        assert!(key.private_key_to_pem_pkcs8().is_ok());

        // DER format
        let der = PKey::private_key_from_pem(TEST_KEY_PEM.as_bytes())
            .unwrap()
            .private_key_to_der()
            .unwrap();
        let value = ValueRef::Binary(&der);
        let key = as_openssl_private_key(&value).unwrap();
        assert!(key.private_key_to_pem_pkcs8().is_ok());
    }

    #[test]
    fn as_openssl_private_key_err() {
        // Invalid type
        let value = ValueRef::F64(std::f64::consts::PI);
        let result = as_openssl_private_key(&value);
        assert!(result.is_err());

        // Invalid PEM
        let value = ValueRef::String("invalid key".into());
        let result = as_openssl_private_key(&value);
        assert!(result.is_err());

        // Invalid DER
        let value = ValueRef::Binary(&[9, 8, 7, 6]); // random invalid DER
        let result = as_openssl_private_key(&value);
        assert!(result.is_err());

        // Empty binary data
        let value = ValueRef::Binary(&[]);
        let result = as_openssl_private_key(&value);
        assert!(result.is_err());
    }
}
