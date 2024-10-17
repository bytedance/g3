/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
    assert!(inspector
        .check_client_initial_data(&config, 443, &data)
        .is_err());
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
    assert!(inspector
        .check_client_initial_data(&config, 443, &data)
        .is_err());
    data.extend_from_slice(RECORD_2_BYTES);
    assert!(inspector
        .check_client_initial_data(&config, 443, &data)
        .is_err());
    data.extend_from_slice(RECORD_3_BYTES);
    let protocol = inspector
        .check_client_initial_data(&config, 443, &data)
        .unwrap();
    assert_eq!(protocol, Protocol::TlsModern);
}
