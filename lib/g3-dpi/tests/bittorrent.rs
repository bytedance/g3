/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port6881_demo() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"\x13\
        BitTorrent protocol\
        \x00\x00\x00\x00\x00\x00\x00\x00\
        \x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\
        \x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02";

    let protocol = inspector
        .check_client_initial_data(&config, 6881, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::BitTorrentOverTcp);
}
