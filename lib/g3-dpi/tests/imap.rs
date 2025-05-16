/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port143_outlook() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"* OK The Microsoft Exchange IMAP4 service is ready. [UwBJADIAUABSADAANABDAEEAMAAwADAANQAuAGEAcABjAHAAcgBkADAANAAuAHAAcgBvAGQALgBvAHUAdABsAG8AbwBrAC4AYwBvAG0A]\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 143, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Imap);
}
