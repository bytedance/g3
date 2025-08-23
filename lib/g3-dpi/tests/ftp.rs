/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check FTP server greeting
fn check_ftp_greeting(data: &[u8], port: u16) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Ftp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_server_initial_data(&config, port, data)
}

// Tests for valid FTP responses
#[test]
fn port21_220_gnu() {
    const DATA: &[u8] = b"220 GNU FTP server ready.\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}

#[test]
fn port21_120_service_ready() {
    const DATA: &[u8] = b"120 Service ready\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}

#[test]
fn port21_421_too_many_users() {
    const DATA: &[u8] = b"421 Too many users, try later\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}

// Tests for invalid responses
#[test]
fn port21_200_invalid_response() {
    const DATA: &[u8] = b"200 Invalid response\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn port21_220_missing_newline() {
    const DATA: &[u8] = b"220 Server ready";
    let result = check_ftp_greeting(DATA, 21);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn port21_220_invalid_suffix() {
    const DATA: &[u8] = b"220XInvalid suffix\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Tests for data length boundaries
#[test]
fn insufficient_data_less_than_min() {
    const DATA: &[u8] = b"123";
    let result = check_ftp_greeting(DATA, 21);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(3))));
}

// Test protocol exclusion through behavior
#[test]
fn excludes_other_protocols_after_detection() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // First detect FTP
    const FTP_DATA: &[u8] = b"220 FTP Server\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 21, FTP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::FtpControl);

    // Now try to detect SSH (should be excluded)
    const SSH_DATA: &[u8] = b"SSH-2.0-OpenSSH";
    let protocol = inspector
        .check_server_initial_data(&config, 22, SSH_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Test multi-line response
#[test]
fn port21_multi_line_response() {
    const DATA: &[u8] = b"220-FTP Server\r\n220 Ready\r\n";
    let protocol = check_ftp_greeting(DATA, 21).unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}

// Test long response handling
#[test]
fn long_response_handling() {
    // Create response that's 600 bytes long
    let mut data = Vec::from("220 ");
    data.extend(vec![b'A'; 596]);
    data.extend(b"\r\n");

    // Should be detected as FTP despite length
    let protocol = check_ftp_greeting(&data, 21).unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}
