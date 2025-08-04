use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to test DNS protocol detection
fn check_dns_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Dns);
    let config = ProtocolInspectionConfig::default();
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn check_dns_tcp_request_message_valid_dns_request() {
    // Valid DNS request: length=12, QR=0, counts=0 (18 bytes total)
    let data = vec![
        0x00, 0x0C, // length = 12
        0x00, 0x00, // transaction ID
        0x00, // flags: QR=0
        0x00, // flags
        0x00, 0x00, // questions
        0x00, 0x00, // answer RRs
        0x00, 0x00, // authority RRs
        0x00, 0x00, // additional RRs
        0x00, 0x00, 0x00, 0x00,
    ];
    let protocol = check_dns_data(&data).expect("Valid DNS request should be detected");
    assert_eq!(protocol, Protocol::Dns);
}

#[test]
fn check_dns_tcp_request_message_valid_with_questions() {
    // Valid: Questions count != 0 (should be allowed) (18 bytes total)
    let data = vec![
        0x00, 0x0C, // length = 12
        0x00, 0x00, // transaction ID
        0x00, // flags: QR=0
        0x00, // flags
        0x00, 0x01, // questions = 1
        0x00, 0x00, // answer RRs
        0x00, 0x00, // authority RRs
        0x00, 0x00, // additional RRs
        0x00, 0x00, 0x00, 0x00,
    ];
    let protocol = check_dns_data(&data).expect("Valid DNS request should be detected");
    assert_eq!(protocol, Protocol::Dns);
}

#[test]
fn check_dns_tcp_request_message_insufficient_data() {
    // Only 10 bytes, less than MIN_DNS_TCP_DATA_LEN (14)
    let data = vec![0x00; 10];
    let err = check_dns_data(&data).expect_err("Should return NeedMoreData error");

    let ProtocolInspectError::NeedMoreData(needed) = err;
    assert_eq!(needed, 1);
}

#[test]
fn check_dns_tcp_request_message_invalid_message_length() {
    // Message length (10) < DNS header length (12) (18 bytes total)
    let data = vec![
        0x00, 0x0A, // length = 10 (<12)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    let protocol = check_dns_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn check_dns_tcp_request_message_qr_bit_set() {
    // Invalid: QR bit set (response packet) (18 bytes total)
    let data = vec![
        0x00, 0x0C, // length = 12
        0x00, 0x00, // transaction ID
        0x80, // flags: QR=1 (invalid for request)
        0x00, // flags
        0x00, 0x00, // questions
        0x00, 0x00, // answer RRs
        0x00, 0x00, // authority RRs
        0x00, 0x00, // additional RRs
        0x00, 0x00, 0x00, 0x00,
    ];
    let protocol = check_dns_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn check_dns_tcp_request_message_non_zero_counts() {
    // Invalid: Non-zero count fields (18 bytes total)
    let data = vec![
        0x00, 0x0C, // length = 12
        0x00, 0x00, // transaction ID
        0x00, // flags: QR=0
        0x00, // flags
        0x00, 0x01, // questions = 1 (valid)
        0x00, 0x01, // answer RRs = 1 (invalid)
        0x00, 0x00, // authority RRs
        0x00, 0x00, // additional RRs
        0x00, 0x00, 0x00, 0x00,
    ];
    let protocol = check_dns_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(protocol, Protocol::Unknown);
}
