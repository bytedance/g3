use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

// Helper function to reduce boilerplate for testing MQTT detection
fn check_mqtt_data(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
    let mut inspector = ProtocolInspector::default();
    // Push MQTT to ensure it's checked first
    inspector.push_protocol(MaybeProtocol::Mqtt);
    let config = ProtocolInspectionConfig::default();
    // Using server_port = 0 as it's not relevant for this specific pushed protocol check
    inspector.check_client_initial_data(&config, 0, data)
}

#[test]
fn check_mqtt_client_connect_request_valid_v4() {
    // Fixed header (0x10) + remaining len=10 + protocol name + level 4
    let data = b"\x10\x0A\x00\x04MQTT\x04\x00\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Valid MQTT v4 request should be detected");
    assert_eq!(p, Protocol::Mqtt);
}

#[test]
fn check_mqtt_client_connect_request_valid_v5() {
    // Fixed header (0x10) + remaining len=10 + protocol name + level 5
    let data = b"\x10\x0A\x00\x04MQTT\x05\x00\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Valid MQTT v5 request should be detected");
    assert_eq!(p, Protocol::Mqtt);
}

#[test]
fn check_mqtt_client_connect_request_insufficient_data() {
    // Only 11 bytes (needs 12)
    let data = b"\x10\x0A\x00\x04MQTT\x04\x00\x00";
    let e = check_mqtt_data(data).expect_err("Should return NeedMoreData error");
    assert!(matches!(e, ProtocolInspectError::NeedMoreData(1)));
}

#[test]
fn check_mqtt_client_connect_request_invalid_first_byte() {
    // Invalid first byte (0x00 instead of 0x10)
    let data = b"\x00\x0A\x00\x04MQTT\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_mqtt_client_connect_request_insufficient_remaining_len() {
    // Remaining length too small (9 bytes, needs 10)
    let data = b"\x10\x09\x00\x04MQTT\x04\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_mqtt_client_connect_request_invalid_protocol_name() {
    // Invalid protocol name ("MQTX" instead of "MQTT")
    let data = b"\x10\x0A\x00\x04MQTX\x04\x00\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}

#[test]
fn check_mqtt_client_connect_request_invalid_protocol_level() {
    // Invalid protocol level (0x06)
    let data = b"\x10\x0A\x00\x04MQTT\x06\x00\x00\x00\x00\x00";
    let p = check_mqtt_data(data).expect("Should return Protocol::Unknown");
    assert_eq!(p, Protocol::Unknown);
}
