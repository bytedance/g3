/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn anonymous_simple_bind() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"\x30\x0c\x02\x01\x01\x60\x07\x02\x01\x03\x04\x00\x08\x00";

    let protocol = inspector
        .check_client_initial_data(&config, 389, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Ldap);
}
