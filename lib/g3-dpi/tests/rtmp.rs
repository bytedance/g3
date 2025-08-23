use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to test RTMP protocol detection
fn check_rtmp_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Rtmp);
    let config = ProtocolInspectionConfig::default();
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn check_rtmp_valid_handshake() {
    // Valid RTMP handshake: version=3, 5-9 bytes=0, full 1537 bytes
    let mut data = vec![0x03]; // RTMP version
    data.extend_from_slice(&[0; 4]); // padding
    data.extend_from_slice(&[0; 4]); // 5-9 bytes must be zero
    data.resize(1537, 0); // full handshake length

    let protocol = check_rtmp_data(&data).expect("Valid RTMP handshake should be detected");
    assert_eq!(protocol, Protocol::RtmpOverTcp);
}

#[test]
fn check_rtmp_insufficient_data_less_than_9() {
    // Only 8 bytes, less than minimum required (9)
    let data = vec![0x03; 8];
    let err = check_rtmp_data(&data).expect_err("Should return NeedMoreData error");

    let ProtocolInspectError::NeedMoreData(needed) = err;
    assert_eq!(needed, 3);
}

#[test]
fn check_rtmp_invalid_version() {
    // Invalid version byte (0x04 instead of 0x03)
    let mut data = vec![0x04]; // invalid version
    data.extend_from_slice(&[0; 8]); // rest of data
    data.resize(1537, 0);

    let protocol = check_rtmp_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn check_rtmp_non_zero_bytes() {
    // Non-zero in bytes 5-9
    let mut data = vec![0x03]; // valid version
    data.extend_from_slice(&[0; 4]); // padding
    data.extend_from_slice(&[1, 0, 0, 0]); // byte5=1 (invalid)
    data.resize(1537, 0);

    let protocol = check_rtmp_data(&data).expect("Should return Protocol::Unknown");
    assert_eq!(protocol, Protocol::Unknown);
}

#[test]
fn check_rtmp_insufficient_data_after_initial() {
    // Valid initial 9 bytes but less than full 1537
    let mut data = vec![0x03]; // version
    data.extend_from_slice(&[0; 4]); // padding
    data.extend_from_slice(&[0; 4]); // 5-9 bytes
    // Stop at 1536 bytes (1 byte short)
    data.resize(1536, 0);

    let err = check_rtmp_data(&data).expect_err("Should return NeedMoreData error");

    let ProtocolInspectError::NeedMoreData(needed) = err;
    assert_eq!(needed, 1);
}

#[test]
fn check_rtmp_protocol_exclusion() {
    // Verify protocol exclusion logic
    let mut data = vec![0x03]; // valid version
    data.extend_from_slice(&[0; 4]);
    data.extend_from_slice(&[0; 4]);
    data.resize(1537, 0);

    let mut inspector = ProtocolInspector::default();
    inspector.push_protocol(MaybeProtocol::Rtmp);
    inspector.push_protocol(MaybeProtocol::Ssh); // should be excluded
    inspector.push_protocol(MaybeProtocol::Http); // should be excluded
    let config = ProtocolInspectionConfig::default();

    let protocol = inspector
        .check_client_initial_data(&config, 0, &data)
        .expect("Should detect RTMP");

    assert_eq!(protocol, Protocol::RtmpOverTcp);

    // Verify SSH and HTTP were excluded
    let mut inspector2 = ProtocolInspector::default();
    inspector2.push_protocol(MaybeProtocol::Ssh);
    inspector2.push_protocol(MaybeProtocol::Http);
    inspector2.push_protocol(MaybeProtocol::Rtmp);

    // Should skip SSH/HTTP and go directly to RTMP
    let protocol2 = inspector2
        .check_client_initial_data(&config, 0, &data)
        .expect("Should detect RTMP");
    assert_eq!(protocol2, Protocol::RtmpOverTcp);
}

#[test]
fn check_rtmp_edge_cases() {
    // Exactly minimum length (9 bytes)
    let mut data = vec![0x03]; // version
    data.extend_from_slice(&[0; 4]);
    data.extend_from_slice(&[0; 4]); // 9 bytes total

    let err = check_rtmp_data(&data).expect_err("Should return NeedMoreData error");

    let ProtocolInspectError::NeedMoreData(needed) = err;
    assert_eq!(needed, 1528); // 1537 - 9 = 1528

    // Valid data with exactly 1537 bytes
    let mut data = vec![0x03];
    data.extend_from_slice(&[0; 1536]); // total 1537 bytes
    let protocol = check_rtmp_data(&data).expect("Valid RTMP handshake");
    assert_eq!(protocol, Protocol::RtmpOverTcp);
}
