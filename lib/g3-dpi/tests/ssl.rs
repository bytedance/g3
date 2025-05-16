/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port443_demo() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"\x16\x03\x01\x02\x00\x01\x00\x01\xfc\x03\x03";

    let protocol = inspector
        .check_client_initial_data(&config, 443, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::TlsModern);
}

#[test]
fn fragmented_one() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const RECORD_1_BYTES: &[u8] = &[
        0x16, 0x03, 0x01, 0x00, 0x64, 0x01, 0x00, 0x01, 0x8a, 0x03, 0x03,
    ];

    let protocol = inspector
        .check_client_initial_data(&config, 443, RECORD_1_BYTES)
        .unwrap();
    assert_eq!(protocol, Protocol::TlsModern);
}

#[test]
fn fragmented_two() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const RECORD_1_BYTES: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x04, 0x01, 0x00, 0x01, 0x8a];
    const RECORD_2_BYTES: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x04, 0x03, 0x03, 0x76, 0xf4];

    let mut data = Vec::new();
    data.extend_from_slice(RECORD_1_BYTES);
    assert!(
        inspector
            .check_client_initial_data(&config, 443, &data)
            .is_err()
    );
    data.extend_from_slice(RECORD_2_BYTES);
    let protocol = inspector
        .check_client_initial_data(&config, 443, &data)
        .unwrap();
    assert_eq!(protocol, Protocol::TlsModern);
}

#[test]
fn fragmented_three() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const RECORD_1_BYTES: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x02, 0x01, 0x00];
    const RECORD_2_BYTES: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x02, 0x01, 0x8a];
    const RECORD_3_BYTES: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x02, 0x03, 0x03];

    let mut data = Vec::new();
    data.extend_from_slice(RECORD_1_BYTES);
    assert!(
        inspector
            .check_client_initial_data(&config, 443, &data)
            .is_err()
    );
    data.extend_from_slice(RECORD_2_BYTES);
    assert!(
        inspector
            .check_client_initial_data(&config, 443, &data)
            .is_err()
    );
    data.extend_from_slice(RECORD_3_BYTES);
    let protocol = inspector
        .check_client_initial_data(&config, 443, &data)
        .unwrap();
    assert_eq!(protocol, Protocol::TlsModern);
}
