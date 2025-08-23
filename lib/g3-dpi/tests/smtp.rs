/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to check SMTP server greeting
fn check_smtp_server_greeting(data: &[u8], port: u16) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Smtp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_server_initial_data(&config, port, data)
}

#[test]
fn valid_220_response() {
    const DATA: &[u8] =
        b"220 smtp.gmail.com ESMTP 68-20020a620647000000b00545832dd969sm8803280pfg.145 - gsmtp\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Smtp);
}

#[test]
fn valid_554_response() {
    const DATA: &[u8] = b"554 Transaction failed\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Smtp);
}

#[test]
fn invalid_second_byte() {
    const DATA: &[u8] = b"230 Invalid response\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_third_byte() {
    const DATA: &[u8] = b"225 No new messages\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn non_554_five_series() {
    const DATA: &[u8] = b"550 Invalid user\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn non_2_or_5_first_byte() {
    let mut data = vec![b'3'; 600];
    data.extend(b"\r\n");
    let protocol = check_smtp_server_greeting(&data, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn invalid_separator_after_code() {
    const DATA: &[u8] = b"220XInvalid response\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn newline_at_start() {
    const DATA: &[u8] = b"554   \n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn missing_cr_before_lf() {
    const DATA: &[u8] = b"554 example.com\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn valid_ipv4_host() {
    const DATA: &[u8] = b"220 192.0.2.1\r\n";
    let protocol = check_smtp_server_greeting(DATA, 25).unwrap();
    assert_eq!(protocol, Protocol::Smtp);
}

#[test]
fn line_too_short() {
    const DATA: &[u8] = b"554\r\n";
    let result = check_smtp_server_greeting(DATA, 25);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}

#[test]
fn line_too_long() {
    let mut data = vec![b'2'; 600];
    data.extend(b"\r\n");
    let protocol = check_smtp_server_greeting(&data, 25).unwrap();
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn need_more_data() {
    const DATA: &[u8] = b"220 incomplete";
    let result = check_smtp_server_greeting(DATA, 25);
    assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));
}
