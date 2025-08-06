/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port80_small() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"GET / HTTP/1.1\r\nHost: www.example.com\r\nUser-Agent: curl/7.85.0\r\n\r\n";

    let protocol = inspector
        .check_client_initial_data(&config, 80, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Http1);
}

#[test]
fn http2_connection_preface() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

    let protocol = inspector
        .check_client_initial_data(&config, 80, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Http2);
}

#[test]
fn http_methods() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    let methods = [
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE", "PATCH",
    ];

    for method in methods {
        let request = format!("{method} / HTTP/1.1\r\nHost: example.com\r\n\r\n");
        let protocol = inspector
            .check_client_initial_data(&config, 80, request.as_bytes())
            .unwrap();
        assert_eq!(protocol, Protocol::Http1);
        inspector.reset_state(); // Reset for next test
    }
}

#[test]
fn insufficient_data() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // Minimum data length is 18, provide only 17 bytes
    const DATA: &[u8] = b"GET / HTTP/1.1\r\nH";

    let result = inspector.check_client_initial_data(&config, 80, DATA);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn invalid_http_version() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"GET / HTTP/1.2\r\nHost: example.com\r\n\r\n";

    let protocol = inspector
        .check_client_initial_data(&config, 80, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn non_standard_port() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";

    // Should detect HTTP even on non-80 port
    let protocol = inspector
        .check_client_initial_data(&config, 8080, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Http1);
}

#[test]
fn uri_too_long() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DEFAULT_URI_LIMIT: usize = 4096;
    let long_uri = "a".repeat(DEFAULT_URI_LIMIT + 10);
    let request = format!("GET /{long_uri} HTTP/1.1\r\nHost: example.com\r\n\r\n");

    let protocol = inspector
        .check_client_initial_data(&config, 80, request.as_bytes())
        .unwrap();
    assert_eq!(protocol, Protocol::Http1);
}

#[test]
fn special_methods() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    let methods = [
        "ACL",
        "BIND",
        "COPY",
        "CHECKIN",
        "CHECKOUT",
        "LOCK",
        "LINK",
        "MOVE",
        "MKCOL",
        "MERGE",
        "MKACTIVITY",
        "MKREDIRECTREF",
    ];

    for method in methods {
        let request = format!("{method} / HTTP/1.1\r\nHost: example.com\r\n\r\n");
        let protocol = inspector
            .check_client_initial_data(&config, 80, request.as_bytes())
            .unwrap();
        assert_eq!(protocol, Protocol::Http1);
        inspector.reset_state();
    }
}

#[test]
fn invalid_method_prefix() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DEFAULT_URI_LIMIT: usize = 4096;
    let long_uri = "a".repeat(DEFAULT_URI_LIMIT + 10);
    let request = format!("XET /{long_uri} HTTP/1.1\r\nHost: example.com\r\n\r\n");

    let protocol = inspector
        .check_client_initial_data(&config, 80, request.as_bytes())
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn protocol_exclusion() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // First detect HTTP
    const HTTP_DATA: &[u8] = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let protocol = inspector
        .check_client_initial_data(&config, 80, HTTP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Http1);

    // Now try SSH (should be excluded)
    const SSH_DATA: &[u8] = b"SSH-2.0-OpenSSH\r\n\r\n";
    let protocol = inspector
        .check_client_initial_data(&config, 22, SSH_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}
