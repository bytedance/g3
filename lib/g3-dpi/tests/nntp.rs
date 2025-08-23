/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check NNTP server greeting
fn check_nntp_greeting(data: &[u8], port: u16) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    // Prioritize NNTP detection
    inspector.push_protocol(MaybeProtocol::Nntp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_server_initial_data(&config, port, data)
}

// Tests for valid NNTP responses
#[test]
fn port119_valid_response() {
    const DATA: &[u8] =
        b"200 news.gmane.io InterNetNews NNRP server INN 2.6.3 ready (posting ok)\r\n";
    let protocol = check_nntp_greeting(DATA, 119).unwrap();
    assert_eq!(protocol, Protocol::Nntp);
}

// Tests for data length boundaries
#[test]
fn insufficient_data_less_than_min() {
    const DATA: &[u8] = b"1234"; // 4 bytes < min 5
    let result = check_nntp_greeting(DATA, 119);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn excess_data_over_max() {
    // Create 513-byte response
    let mut data = Vec::from("200 ");
    data.extend(vec![b'A'; 509]); // 4 + 509 = 513 bytes
    let protocol = check_nntp_greeting(&data, 119).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Tests for byte sequence validation
#[test]
fn invalid_first_byte() {
    // First byte not '2' (0x32)
    let mut data = Vec::from("300 Invalid response\r\n");
    data.extend(vec![b'A'; 100]);
    let protocol = check_nntp_greeting(&data, 119).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_second_byte() {
    // Second byte not '0'
    const DATA: &[u8] = b"230 Invalid response\r\n";
    let protocol = check_nntp_greeting(DATA, 119).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_third_byte() {
    // Third byte not '0' or '1'
    const DATA: &[u8] = b"202 Invalid response\r\n"; // Third byte '2'
    let protocol = check_nntp_greeting(DATA, 119).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_carriage_return() {
    // No '\r' before '\n'
    const DATA: &[u8] = b"200 Server ready\n";
    let protocol = check_nntp_greeting(DATA, 119).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_line_feed() {
    // Ends with '\r' but no '\n'
    const DATA: &[u8] = b"200 Server ready\r";
    let result = check_nntp_greeting(DATA, 119);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

// Test protocol exclusion
#[test]
fn excludes_protocols_after_checks() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // First check NNTP response
    const NNTP_DATA: &[u8] = b"200 Server ready\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 119, NNTP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Nntp);

    // Now check SSH (should be excluded)
    const SSH_DATA: &[u8] = b"SSH-2.0-OpenSSH";
    let protocol = inspector
        .check_server_initial_data(&config, 22, SSH_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}
