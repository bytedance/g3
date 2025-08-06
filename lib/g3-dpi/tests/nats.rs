/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check NATS server info message
fn check_nats_data(
    data: &[u8],
    port: u16,
    size_limit: Option<usize>,
) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    let mut config = ProtocolInspectionConfig::default();

    // Apply custom size limit if provided
    if let Some(limit) = size_limit {
        config.size_limit_mut().set_nats_server_info_line(limit);
    }

    inspector.push_protocol(MaybeProtocol::Nats);
    inspector.check_server_initial_data(&config, port, data)
}

#[test]
fn port4222_demo() {
    const DATA: &[u8] =
        b"INFO {\"server_id\":\"Zk0GQ3JBSrg3oyxCRRlE09\",\"version\":\"1.2.0\",\"proto\":1,\"go\":\"go1.10.3\",\"host\":\"0.0.0.0\",\"port\":4222,\"max_payload\":1048576,\"client_id\":2392}\r\n";
    let protocol = check_nats_data(DATA, 4222, None).unwrap();
    assert_eq!(protocol, Protocol::Nats);
}

#[test]
fn insufficient_data() {
    // Data length < MINIMUM_DATA_LEN(10)
    const DATA: &[u8] = b"INFO {";
    let result = check_nats_data(DATA, 4222, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(4))));
}

#[test]
fn invalid_first_byte() {
    // First byte not 'I'
    const DATA: &[u8] =
        b"XINFO {\"server_id\":\"Zk0GQ3JBSrg3oyxCRRlE09\",\"version\":\"1.2.0\",\"proto\":1,\"go\":\"go1.10.3\",\"host\":\"0.0.0.0\",\"port\":4222,\"max_payload\":1048576,\"client_id\":2392}\r\n";
    let protocol = check_nats_data(DATA, 4222, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_prefix() {
    // Does not start with "INFO {"
    const DATA: &[u8] = b"INFOX{\"test\":1}\r\n";
    let protocol = check_nats_data(DATA, 4222, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_newline() {
    // No newline at end (within size limit)
    const DATA: &[u8] = b"INFO {\"test\":1}";
    let result = check_nats_data(DATA, 4222, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn missing_newline_exceeds_limit() {
    // No newline at end (exceeds size limit)
    let mut data = vec![0u8; 1025]; // Exceeds default 1024 limit
    data[0..6].copy_from_slice(b"INFO {");
    let protocol = check_nats_data(&data, 4222, Some(10)).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_carriage_return() {
    // Ends with '\n' but missing preceding '\r'
    const DATA: &[u8] = b"INFO {\"test\":1}\n";
    let protocol = check_nats_data(DATA, 4222, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn valid_minimal_data() {
    // Minimal valid data with \r\n ending
    const DATA: &[u8] = b"INFO { }\r\n";
    let protocol = check_nats_data(DATA, 4222, None).unwrap();
    assert_eq!(protocol, Protocol::Nats);
}
