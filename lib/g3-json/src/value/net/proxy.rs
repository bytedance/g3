/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::net::ProxyRequestType;

pub fn as_proxy_request_type(v: &Value) -> anyhow::Result<ProxyRequestType> {
    if let Value::String(s) = v {
        let t = ProxyRequestType::from_str(s)
            .map_err(|_| anyhow!("invalid 'ProxyRequestType' value"))?;
        Ok(t)
    } else {
        Err(anyhow!(
            "json value type for 'ProxyRequestType' should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_proxy_request_type_ok() {
        // all valid enum values and their supported string formats
        let test_cases = vec![
            ("httpforward", ProxyRequestType::HttpForward),
            ("HTTPForward", ProxyRequestType::HttpForward),
            ("http_forward", ProxyRequestType::HttpForward),
            ("httpsforward", ProxyRequestType::HttpsForward),
            ("HTTPSForward", ProxyRequestType::HttpsForward),
            ("https_forward", ProxyRequestType::HttpsForward),
            ("ftpoverhttp", ProxyRequestType::FtpOverHttp),
            ("FTPOverHttp", ProxyRequestType::FtpOverHttp),
            ("ftp_over_http", ProxyRequestType::FtpOverHttp),
            ("httpconnect", ProxyRequestType::HttpConnect),
            ("HTTPConnect", ProxyRequestType::HttpConnect),
            ("http_connect", ProxyRequestType::HttpConnect),
            ("sockstcpconnect", ProxyRequestType::SocksTcpConnect),
            ("SocksTCPConnect", ProxyRequestType::SocksTcpConnect),
            ("socks_tcp_connect", ProxyRequestType::SocksTcpConnect),
            ("socksudpassociate", ProxyRequestType::SocksUdpAssociate),
            ("SocksUDPAssociate", ProxyRequestType::SocksUdpAssociate),
            ("socks_udp_associate", ProxyRequestType::SocksUdpAssociate),
        ];

        for (input, expected) in test_cases {
            let value = Value::String(input.to_string());
            let result = as_proxy_request_type(&value).unwrap();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn as_proxy_request_type_err() {
        // invalid string
        let invalid_str = Value::String("invalid_type".to_string());
        assert!(as_proxy_request_type(&invalid_str).is_err());

        // non-string types
        let non_string_types = vec![
            Value::Bool(true),
            Value::Number(42.into()),
            Value::Null,
            Value::Array(vec![]),
            Value::Object(serde_json::Map::new()),
        ];

        for value in non_string_types {
            assert!(as_proxy_request_type(&value).is_err());
        }
    }
}
