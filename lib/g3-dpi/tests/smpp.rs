use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Constants matching the implementation
const SMPP_BIND_MIN_BODY: usize = 7;
const SMPP_BIND_MAX_BODY: usize = 16 + 9 + 13 + 1 + 1 + 1 + 41; // 41 bytes
const SMPP_OUTBIND_MIN_BODY: usize = 2;
const SMPP_OUTBIND_MAX_BODY: usize = 16 + 9;
const SMPP_SESSION_REQUEST_HEADER_LEN: usize = 16;

// Helper function to test SMPP detection
fn check_smpp_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Smpp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn valid_bind_transmitter_request() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MIN_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes()); // BIND_TRANSMITTER
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid BIND_TRANSMITTER should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn valid_bind_receiver_request() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MIN_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000001u32.to_be_bytes()); // BIND_RECEIVER
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid BIND_RECEIVER should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn valid_bind_transceiver_request() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MIN_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000009u32.to_be_bytes()); // BIND_TRANSCEIVER
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid BIND_TRANSCEIVER should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn valid_outbind_request() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_OUTBIND_MIN_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x0000000Bu32.to_be_bytes()); // OUTBIND
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid OUTBIND should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn valid_bind_max_body() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MAX_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes()); // BIND_TRANSMITTER
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid max body should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn valid_outbind_max_body() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_OUTBIND_MAX_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x0000000Bu32.to_be_bytes()); // OUTBIND
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).expect("Valid max body should be detected");
    assert_eq!(p, Protocol::Smpp);
}

#[test]
fn insufficient_data() {
    let data = vec![0x00; SMPP_SESSION_REQUEST_HEADER_LEN - 1];
    let err = check_smpp_data(&data).expect_err("Should require more data");
    assert!(matches!(err, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn invalid_first_byte() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MAX_BODY;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes());
    data[8..12].copy_from_slice(&0u32.to_be_bytes());
    data[0] = 0x01; // Invalid first byte

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn invalid_command_length() {
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + SMPP_BIND_MIN_BODY;
    let mut data = vec![0x00; total_len];
    // Set invalid command length (less than header length)
    data[0..4].copy_from_slice(&(SMPP_SESSION_REQUEST_HEADER_LEN as u32 - 1).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes());
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn invalid_command_id() {
    let body_len = SMPP_BIND_MIN_BODY;
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + body_len;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // Invalid ID
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn body_length_below_min() {
    let body_len = SMPP_BIND_MIN_BODY - 1;
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + body_len;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes());
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn body_length_above_max() {
    let body_len = SMPP_BIND_MAX_BODY + 1;
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + body_len;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes());
    data[8..12].copy_from_slice(&0u32.to_be_bytes());

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn non_zero_command_status() {
    let body_len = SMPP_BIND_MIN_BODY;
    let total_len = SMPP_SESSION_REQUEST_HEADER_LEN + body_len;
    let mut data = vec![0x00; total_len];
    data[0..4].copy_from_slice(&(total_len as u32).to_be_bytes());
    data[4..8].copy_from_slice(&0x00000002u32.to_be_bytes());
    data[8..12].copy_from_slice(&1u32.to_be_bytes()); // Non-zero status

    let p = check_smpp_data(&data).unwrap();
    assert_eq!(p, Protocol::Unknown);
}
