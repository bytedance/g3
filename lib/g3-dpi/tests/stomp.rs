use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// This constant is based on MINIMUM_DATA_LEN in lib/g3-dpi/src/protocol/stomp.rs
const MIN_STOMP_CLIENT_HANDSHAKE_LEN: usize = 10;

// Helper function to reduce boilerplate for testing STOMP detection
fn check_stomp_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    // Push Stomp to ensure it's checked, prioritizing it.
    // This makes the test more specific to STOMP detection logic.
    inspector.push_protocol(MaybeProtocol::Stomp);
    let config = ProtocolInspectionConfig::default();
    // Using server_port = 0 as it's not relevant for this specific pushed protocol check.
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn check_stomp_client_connect_request_valid_connect() {
    // CONNECT\n\n\0
    let data = b"CONNECT\n\n\0"; // 10 bytes
    let p = check_stomp_data(data).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_client_connect_request_valid_stomp() {
    // STOMP\naccept-version:1.2\nhost:stomp.github.com\n\n\0
    let data = b"STOMP\naccept-version:1.2\nhost:stomp.github.com\n\n\0"; // 51 bytes
    let p = check_stomp_data(data).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_client_connect_request_invalid_method_initial() {
    let data = b"IN\n\n\0";
    let e = check_stomp_data(data).expect_err("Should be invalid STOMP protocol");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(5)));
}

#[test]
fn check_stomp_client_connect_request_need_more_data_very_short() {
    let data = b"CO"; // 2 bytes, shorter than MIN_STOMP_CLIENT_HANDSHAKE_LEN
    let ProtocolInspectError::NeedMoreData(needed) = check_stomp_data(data)
        .expect_err("Protocol check with insufficient data should return NeedMoreData error");

    let expected_max = MIN_STOMP_CLIENT_HANDSHAKE_LEN - data.len();
    assert!(
        needed > 0 && needed <= expected_max,
        "Needed {} bytes, data len {}, expected > 0 and <= {}",
        needed,
        data.len(),
        expected_max
    );
}

#[test]
fn check_stomp_client_connect_request_invalid_c_prefix() {
    let data = b"CONNEXT_IS_INVALID_AND_LONG";
    let p = check_stomp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_client_connect_request_invalid_s_prefix() {
    let data = b"STOMX_IS_INVALID_AND_LONG_TOO";
    let p = check_stomp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_connect_method_valid_lf() {
    let data = b"CONNECT\n\0\0";
    let p = check_stomp_data(data).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_connect_method_valid_crlf() {
    // CONNECT\r\n + padding
    // The actual STOMP logic for "CONNECT\r\n" needs 11 bytes (offset 9 + 2).
    let data = b"CONNECT\r\n\0"; // 10 bytes. This will be NeedMoreData(1)
    let e = check_stomp_data(data)
        .expect_err("Protocol check with insufficient data should return NeedMoreData");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(1)));

    let data_long_enough = b"CONNECT\r\n\0\0"; // 11 bytes
    let p = check_stomp_data(data_long_enough).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_connect_method_invalid_command_suffix() {
    let data = b"CONNECX\n\n\0"; // 10 bytes, Invalid STOMP command
    let e = check_stomp_data(data)
        .expect_err("Protocol with invalid command should return Protocol::Unknown");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(8)));
}

#[test]
fn check_stomp_connect_method_invalid_line_ending_after_command() {
    // "CONNECTX" where X is not \n or \r
    let data = b"CONNECTX\n\n\0"; // 11 bytes
    let e = check_stomp_data(data)
        .expect_err("Protocol with invalid line ending should return Protocol::Unknown");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(7)));
}

#[test]
fn check_stomp_connect_method_invalid_crlf_variant_after_command() {
    // "CONNECT\rX" where X is not \n
    let data = b"CONNECT\rX\n\0"; // 11 bytes
    let p = check_stomp_data(data).expect("Expected Ok, got Err");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_connect_method_need_more_data_for_initial_check_lf_style() {
    // Test "CONNECT\n" like prefixes that are too short for MIN_STOMP_CLIENT_HANDSHAKE_LEN (10 bytes)
    let data = b"CONNECT\n"; // 8 bytes
    let e = check_stomp_data(data)
        .expect_err("Protocol check with insufficient data should return NeedMoreData");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(2)));

    let data2 = b"CONNECT\nX"; // 9 bytes
    let e2 = check_stomp_data(data2)
        .expect_err("Protocol check with insufficient data should return NeedMoreData");
    assert!(matches!(e2, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn check_stomp_connect_method_need_more_data_after_method_crlf() {
    // For "CONNECT\r\n", offset is 9. MINIMUM_LEN_AFTER_METHOD is 2. Needs 11 bytes.
    let data = b"CONNECT\r\nA"; // 10 bytes
    let e = check_stomp_data(data)
        .expect_err("Protocol check with insufficient data should return NeedMoreData");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn check_stomp_stomp_method_valid_lf() {
    let data = b"STOMP\naccept-version:1.2\nhost:x\n\n\0";
    let p = check_stomp_data(data).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_stomp_method_valid_crlf() {
    let data = b"STOMP\r\naccept-version:1.2\nhost:x\n\n\0";
    let p = check_stomp_data(data).expect("Should be valid STOMP protocol");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_stomp_method_invalid_command_suffix() {
    let data = b"STOMX\naccept-version:1.2\nhost:x\n\n\0";
    let p = check_stomp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_stomp_method_invalid_line_ending_after_command() {
    let data = b"STOMPXaccept-version:1.2\nhost:x\n\n\0";
    let p = check_stomp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_stomp_method_invalid_crlf_variant_after_command() {
    let data = b"STOMP\rXaccept-version:1.2\nhost:x\n\n\0";
    let p = check_stomp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_stomp_stomp_method_need_more_data_after_method_lf() {
    // For "STOMP\n", offset is 6. MINIMUM_LEN_AFTER_METHOD is 26. Needs 32 bytes.
    // Input "STOMP\n" + "A"*25 (total 6+25 = 31 bytes).
    // Outer check (MIN_STOMP_CLIENT_HANDSHAKE_LEN=10) passes. Inner needs 32. NeedMoreData(1).
    let mut data_vec = b"STOMP\n".to_vec();
    data_vec.extend_from_slice(&[b'A'; 25]); // Total 31 bytes
    assert!(data_vec.len() >= MIN_STOMP_CLIENT_HANDSHAKE_LEN);

    let e = check_stomp_data(&data_vec).expect_err("Expected NeedMoreData, got Ok");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn check_stomp_stomp_method_need_more_data_after_method_crlf() {
    // For "STOMP\r\n", offset is 7. MINIMUM_LEN_AFTER_METHOD is 26. Needs 33 bytes.
    // Input "STOMP\r\n" + "A"*25 (total 7+25 = 32 bytes).
    // Outer check passes. Inner needs 33. NeedMoreData(1).
    let mut data_vec = b"STOMP\r\n".to_vec();
    data_vec.extend_from_slice(&[b'A'; 25]); // Total 32 bytes
    assert!(data_vec.len() >= MIN_STOMP_CLIENT_HANDSHAKE_LEN);

    let e = check_stomp_data(&data_vec).expect_err("Expected NeedMoreData, got Ok");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn check_stomp_client_connect_request_empty_data() {
    let data = b"";
    let e = check_stomp_data(data).expect_err("Expected NeedMoreData, got Ok");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(_)));
}

#[test]
fn check_stomp_client_connect_request_just_enough_for_connect_no_null_termination_check() {
    // "CONNECT\n\nX" is 10 bytes. This should be Ok(Some(Protocol::Stomp))
    // as the TODO in stomp.rs mentions "check header and ending '\0'" is not yet implemented.
    // The current logic only checks the command and initial line endings.
    let data = b"CONNECT\n\nX"; // 10 bytes
    let p = check_stomp_data(data).expect("Result should be Ok");
    assert_eq!(p, Protocol::Stomp);
}

#[test]
fn check_stomp_client_connect_request_just_enough_for_stomp_no_null_termination_check() {
    // "STOMP\n" + "A"*26 (enough for headers as per MINIMUM_LEN_AFTER_METHOD)
    // Total 6 + 26 = 32 bytes.
    // This should be Ok(Some(Protocol::Stomp))
    let mut data_vec = b"STOMP\n".to_vec();
    data_vec.extend_from_slice(&[b'A'; 26]); // Total 32 bytes
    let p = check_stomp_data(&data_vec).expect("Result should be Ok");
    assert_eq!(p, Protocol::Stomp);
}
