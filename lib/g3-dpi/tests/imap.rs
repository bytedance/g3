/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check IMAP server greeting with optional size limit
fn check_imap_greeting(
    data: &[u8],
    size_limit: Option<usize>,
) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    let mut config = ProtocolInspectionConfig::default();

    // Apply custom size limit if provided
    if let Some(limit) = size_limit {
        config.size_limit_mut().set_imap_server_greeting_msg(limit);
    }

    // Push IMAP protocol to ensure it's checked first
    inspector.push_protocol(MaybeProtocol::Imap);
    inspector.check_server_initial_data(&config, 143, data)
}

#[test]
fn port143_outlook() {
    const DATA: &[u8] =
        b"* OK The Microsoft Exchange IMAP4 service is ready. [UwBJADIAUABSADAANABDAEEAMAAwADAANQAuAGEAcABjAHAAcgBkADAANAAuAHAAcgBvAGQALgBvAHUAdABsAG8AbwBrAC4AYwBvAG0A]\r\n";

    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

// Tests for valid IMAP responses
#[test]
fn valid_ok_response() {
    const DATA: &[u8] = b"* OK Server ready\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn valid_preauth_response() {
    const DATA: &[u8] = b"* PREAUTH [CAPABILITY IMAP4rev1 SASL-IR LOGIN-REFERRALS STARTTLS AUTH=PLAIN] Logged in\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn valid_bye_response() {
    const DATA: &[u8] = b"* BYE Autologout; idle for too long\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn valid_multi_line_response() {
    const DATA: &[u8] = b"* OK [CAPABILITY IMAP4rev1]\r\n* PREAUTH\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

// Tests for insufficient data
#[test]
fn insufficient_min_length() {
    const DATA: &[u8] = b"* O";
    let result = check_imap_greeting(DATA, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(3))));
}

#[test]
fn insufficient_preauth_length() {
    const DATA: &[u8] = b"* PREA";
    let result = check_imap_greeting(DATA, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(5))));
}

// Tests for invalid protocol markers
#[test]
fn invalid_first_byte() {
    let mut data = Vec::from("X OK Invalid ");
    data.extend(vec![b'X'; 54]);
    data.extend(b"\r\n");

    let protocol = check_imap_greeting(&data, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_space_after_star() {
    const DATA: &[u8] = b"*XOK Missing space\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Tests for invalid status words
#[test]
fn invalid_status_word() {
    const DATA: &[u8] = b"* INVALID Status\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn incomplete_ok() {
    const DATA: &[u8] = b"* O Missing K\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn incomplete_bye() {
    const DATA: &[u8] = b"* BY Missing E\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Tests for line endings
#[test]
fn missing_lf_ending() {
    const DATA: &[u8] = b"* OK No LF";
    let result = check_imap_greeting(DATA, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn invalid_ending_missing_cr() {
    const DATA: &[u8] = b"* OK Missing CR\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_ending_missing_lf() {
    const DATA: &[u8] = b"* OK Missing LF\r";
    let result = check_imap_greeting(DATA, None);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

// Tests for size limits
#[test]
fn size_limit_exceeded() {
    let mut data = Vec::from("* OK ");
    data.extend(vec![b'X'; 512]);

    let protocol = check_imap_greeting(&data, None).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn size_limit_within_bounds() {
    // Create response that's exactly 512 bytes long
    let mut data = Vec::from("* OK ");
    data.extend(vec![b'X'; 507]); // 5 + 507 = 512 bytes
    data.extend(b"\r\n");

    let protocol = check_imap_greeting(&data, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn custom_size_limit() {
    // Create response that's 600 bytes long but with custom limit
    let mut data = Vec::from("* OK ");
    data.extend(vec![b'X'; 595]); // 5 + 595 = 600 bytes
    data.extend(b"\r\n");

    let protocol = check_imap_greeting(&data, Some(1024)).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

// Test protocol exclusion
#[test]
fn excludes_other_protocols() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    // First detect IMAP
    const IMAP_DATA: &[u8] = b"* OK IMAP4rev1\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 143, IMAP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Imap);

    // Now try to detect FTP (should be excluded)
    const FTP_DATA: &[u8] = b"220 FTP Server\r\n";
    let protocol = inspector
        .check_server_initial_data(&config, 21, FTP_DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

// Test minimum valid responses
#[test]
fn minimum_valid_ok() {
    const DATA: &[u8] = b"* OK\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn minimum_valid_preauth() {
    const DATA: &[u8] = b"* PREAUTH\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}

#[test]
fn minimum_valid_bye() {
    const DATA: &[u8] = b"* BYE\r\n";
    let protocol = check_imap_greeting(DATA, None).unwrap();
    assert_eq!(protocol, Protocol::Imap);
}
