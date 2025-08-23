/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use rmpv::ValueRef;

use g3_types::net::{TlsCertUsage, TlsServiceType};

pub fn as_tls_service_type(v: &ValueRef) -> anyhow::Result<TlsServiceType> {
    match v {
        ValueRef::String(s) => {
            if let Some(s) = s.as_str() {
                TlsServiceType::from_str(s)
                    .map_err(|_| anyhow!("invalid tls service type string {s}"))
            } else {
                Err(anyhow!("invalid utf-8 string"))
            }
        }
        ValueRef::Binary(b) => {
            let s =
                std::str::from_utf8(b).map_err(|e| anyhow!("invalid utf-8 string buffer: {e}"))?;
            TlsServiceType::from_str(s).map_err(|_| anyhow!("invalid tls service type string {s}"))
        }
        ValueRef::Integer(i) => {
            let u = i
                .as_u64()
                .ok_or_else(|| anyhow!("out of range integer value"))?;
            let u = u8::try_from(u).map_err(|e| anyhow!("invalid u8 value: {e}"))?;
            TlsServiceType::try_from(u).map_err(|_| anyhow!("invalid u8 tls server type value {u}"))
        }
        _ => Err(anyhow!(
            "msgpack value type for 'tls service type' should be 'binary' or 'string' or 'u8'"
        )),
    }
}

pub fn as_tls_cert_usage(v: &ValueRef) -> anyhow::Result<TlsCertUsage> {
    match v {
        ValueRef::String(s) => {
            if let Some(s) = s.as_str() {
                TlsCertUsage::from_str(s).map_err(|_| anyhow!("invalid tls cert usage string: {s}"))
            } else {
                Err(anyhow!("invalid utf-8 string"))
            }
        }
        ValueRef::Binary(b) => {
            let s =
                std::str::from_utf8(b).map_err(|e| anyhow!("invalid utf-8 string buffer: {e}"))?;
            TlsCertUsage::from_str(s).map_err(|_| anyhow!("invalid tls cert usage string: {s}"))
        }
        ValueRef::Integer(i) => {
            let u = i
                .as_u64()
                .ok_or_else(|| anyhow!("out of range integer value"))?;
            let u = u8::try_from(u).map_err(|e| anyhow!("invalid u8 value: {e}"))?;
            TlsCertUsage::try_from(u).map_err(|_| anyhow!("invalid u8 tls cert usage value {u}"))
        }
        _ => Err(anyhow!(
            "msgpack value type for 'tls service type' should be 'binary' or 'string' or 'u8'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_tls_service_type_ok() {
        // Valid string inputs
        let v = ValueRef::String("http".into());
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Http);

        let v = ValueRef::String("SMTP".into());
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Smtp);

        // Valid binary inputs
        let v = ValueRef::Binary(b"imap");
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Imap);

        // Valid integer inputs
        let v = ValueRef::Integer(0.into());
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Http);

        let v = ValueRef::Integer(1.into());
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Smtp);

        let v = ValueRef::Integer(2.into());
        assert_eq!(as_tls_service_type(&v).unwrap(), TlsServiceType::Imap);
    }

    #[test]
    fn as_tls_service_type_err() {
        // Invalid string
        let v = ValueRef::String("ftp".into());
        assert!(as_tls_service_type(&v).is_err());

        // Invalid UTF-8 in string
        let v = ValueRef::String("\0ff\0fe".into());
        assert!(as_tls_service_type(&v).is_err());

        // Out-of-range integer
        let v = ValueRef::Integer(3.into());
        assert!(as_tls_service_type(&v).is_err());

        // Invalid UTF-8 in binary
        let v = ValueRef::Binary(&[0xff, 0xff]);
        assert!(as_tls_service_type(&v).is_err());

        // Wrong type (boolean)
        let v = ValueRef::Boolean(true);
        assert!(as_tls_service_type(&v).is_err());
    }

    #[test]
    fn as_tls_cert_usage_ok() {
        // Valid string inputs
        let v = ValueRef::String("tls_server".into());
        assert_eq!(as_tls_cert_usage(&v).unwrap(), TlsCertUsage::TlsServer);

        let v = ValueRef::String("tlsserver".into());
        assert_eq!(as_tls_cert_usage(&v).unwrap(), TlsCertUsage::TlsServer);

        let v = ValueRef::String("tls_server_tongsuo".into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TLsServerTongsuo
        );

        let v = ValueRef::String("tlcp_server_sign".into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TlcpServerSignature
        );

        let v = ValueRef::String("tlcp_server_enc".into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TlcpServerEncryption
        );

        // Valid binary inputs
        let v = ValueRef::Binary(b"tls_server");
        assert_eq!(as_tls_cert_usage(&v).unwrap(), TlsCertUsage::TlsServer);

        // Valid integer inputs
        let v = ValueRef::Integer(0.into());
        assert_eq!(as_tls_cert_usage(&v).unwrap(), TlsCertUsage::TlsServer);

        let v = ValueRef::Integer(1.into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TLsServerTongsuo
        );

        let v = ValueRef::Integer(11.into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TlcpServerSignature
        );

        let v = ValueRef::Integer(12.into());
        assert_eq!(
            as_tls_cert_usage(&v).unwrap(),
            TlsCertUsage::TlcpServerEncryption
        );
    }

    #[test]
    fn as_tls_cert_usage_err() {
        // Invalid string
        let v = ValueRef::String("invalid".into());
        assert!(as_tls_cert_usage(&v).is_err());

        // Invalid UTF-8 in string
        let v = ValueRef::String("\0ff\0fe".into());
        assert!(as_tls_cert_usage(&v).is_err());

        // Out-of-range integer (13)
        let v = ValueRef::Integer(13.into());
        assert!(as_tls_cert_usage(&v).is_err());

        // Unsupported integer (2)
        let v = ValueRef::Integer(2.into());
        assert!(as_tls_cert_usage(&v).is_err());

        // Wrong type (float)
        let v = ValueRef::F64(1.0);
        assert!(as_tls_cert_usage(&v).is_err());
    }
}
