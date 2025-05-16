/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port21_220_gnu() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] = b"220 GNU FTP server ready.\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 21, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::FtpControl);
}
