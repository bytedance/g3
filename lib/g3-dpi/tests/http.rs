/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port80_small() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"GET / HTTP/1.1\r\nHost: www.example.com\r\nUser-Agent: curl/7.85.0\r\n\r\n";

    let protocol = inspector
        .check_client_initial_data(&config, 80, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Http1);
}
