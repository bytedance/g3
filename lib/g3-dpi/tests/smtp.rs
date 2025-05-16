/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port25_220_gmail() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"220 smtp.gmail.com ESMTP 68-20020a620647000000b00545832dd969sm8803280pfg.145 - gsmtp\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 25, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Smtp);
}
