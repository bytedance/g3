/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check POP3 server greeting
fn check_pop3_greeting(data: &[u8], port: u16) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    // Push Pop3 to ensure it's checked first
    inspector.push_protocol(MaybeProtocol::Pop3);
    let config = ProtocolInspectionConfig::default();
    inspector.check_server_initial_data(&config, port, data)
}

#[test]
fn port110_outlook() {
    const DATA: &[u8] =
        b"+OK The Microsoft Exchange POP3 service is ready. [UwBHADIAUABSADAAMwBDAEEAMAAwADkANwAuAGEAcABjAHAAcgBkADAAMwAuAHAAcgBvAGQALgBvAHUAdABsAG8AbwBrAC4AYwBvAG0A]\r\n";
    let protocol = check_pop3_greeting(DATA, 110).unwrap();
    assert_eq!(protocol, Protocol::Pop3);
}

#[test]
fn valid_ok_response() {
    // Test minimal valid POP3 response
    const DATA: &[u8] = b"+OK\r\n";
    let protocol = check_pop3_greeting(DATA, 110).unwrap();
    assert_eq!(protocol, Protocol::Pop3);
}

#[test]
fn valid_response_with_message() {
    // Test response with additional message
    const DATA: &[u8] = b"+OK POP3 server ready\r\n";
    let protocol = check_pop3_greeting(DATA, 110).unwrap();
    assert_eq!(protocol, Protocol::Pop3);
}

#[test]
fn insufficient_data_length() {
    // Test data shorter than minimum required length (5 bytes)
    const DATA: &[u8] = b"+OK";
    let result = check_pop3_greeting(DATA, 110);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(2))));
}

#[test]
fn excessive_data_length() {
    // Test data longer than maximum allowed length (512 bytes)
    let data: Vec<u8> = vec![b'+'; 513];
    let protocol = check_pop3_greeting(&data, 110).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_first_byte() {
    // Test response starting with '-' instead of '+'
    let data: Vec<u8> = vec![b'-'; 512];
    let protocol = check_pop3_greeting(&data, 110).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_ok_keyword() {
    // Test response with '+' but without "OK"
    const DATA: &[u8] = b"+HELLO\r\n";
    let protocol = check_pop3_greeting(DATA, 110).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_line_ending_missing_cr() {
    // Test response with LF but no CR
    const DATA: &[u8] = b"+OK POP3 server ready\n";
    let protocol = check_pop3_greeting(DATA, 110).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_line_ending_missing_lf() {
    // Test response with CR but no LF
    const DATA: &[u8] = b"+OK POP3 server ready\r";
    let result = check_pop3_greeting(DATA, 110);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn protocol_exclusion_after_detection() {
    // Test that POP3 detection excludes other protocols
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // First detect POP3
    const POP3_DATA: &[u8] = b"+OK\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 110, POP3_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Pop3);

    // Now try to detect FTP (should be excluded)
    const FTP_DATA: &[u8] = b"220 FTP Server\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 21, FTP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}
