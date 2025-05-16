/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port22_openssh() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"SSH-2.0-OpenSSH_7.9p1 Debian-10+deb10u2\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 22, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Ssh);
}
