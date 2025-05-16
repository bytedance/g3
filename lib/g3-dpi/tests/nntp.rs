/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port119_gmane() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"200 news.gmane.io InterNetNews NNRP server INN 2.6.3 ready (posting ok)\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 119, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Nntp);
}
