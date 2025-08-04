use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to test RTSP protocol detection
fn check_rtsp_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Rtsp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn check_rtsp_client_setup_request_valid_rtsp_setup_request() {
    // Valid RTSP SETUP request with minimum required length
    let data = b"SETUP rtsp://example.com/media.mp4 RTSP/1.0\r\n";
    let p = check_rtsp_data(data).expect("Valid RTSP should be detected");
    assert_eq!(p, Protocol::Rtsp);
}

#[test]
fn check_rtsp_client_setup_request_insufficient_data_length() {
    // Only 23 bytes (less than required 25)
    let data = b"SETUP rtsp://example.co";
    let err = check_rtsp_data(data).expect_err("Should return NeedMoreData error");

    let ProtocolInspectError::NeedMoreData(needed) = err;
    assert_eq!(needed, 2);
}

#[test]
fn check_rtsp_client_setup_request_invalid_first_byte() {
    // First byte is 'T' instead of required 'S'
    let data = b"TESTUP rtsp://example.com/media.mp4 RTSP/1.0\r\n";
    let p = check_rtsp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_rtsp_client_setup_request_invalid_prefix() {
    // Valid first byte 'S' but invalid prefix
    let data = b"SOMETHING rtsp://example.com/media.mp4 RTSP/1.0\r\n";
    let p = check_rtsp_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_rtsp_client_setup_request_exact_min_length_valid() {
    // Exactly 25 bytes with valid prefix
    let mut data = [0u8; 25];
    data[..13].copy_from_slice(b"SETUP rtsp://");
    let p = check_rtsp_data(&data).expect("Valid RTSP should be detected");
    assert_eq!(p, Protocol::Rtsp);
}

#[test]
fn check_rtsp_client_setup_request_exact_min_length_invalid() {
    // Exactly 25 bytes with invalid prefix
    let mut data = [0u8; 25];
    data[..13].copy_from_slice(b"SETUP ftp:// ");
    let p = check_rtsp_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_rtsp_client_setup_request_protocol_exclusion_verification() {
    // Create inspector with multiple protocols
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Ssl);
    inspector.push_protocol(MaybeProtocol::Http);
    inspector.push_protocol(MaybeProtocol::Rtsp);
    inspector.push_protocol(MaybeProtocol::Mqtt);

    let config = ProtocolInspectionConfig::default();
    let data = b"SETUP rtsp://example.com/media.mp4 RTSP/1.0\r\n";

    // Process valid RTSP data
    let result = inspector
        .check_client_initial_data(&config, 0, data)
        .expect("Should detect RTSP protocol");

    // Verify only RTSP was detected (others should be excluded)
    assert_eq!(result, Protocol::Rtsp);
}
