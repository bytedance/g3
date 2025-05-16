/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{Protocol, ProtocolInspectionConfig, ProtocolInspector};

#[test]
fn port4222_demo() {
    let mut inspector = ProtocolInspector::default();
    let config = ProtocolInspectionConfig::default();

    const DATA: &[u8] =
        b"INFO {\"server_id\":\"Zk0GQ3JBSrg3oyxCRRlE09\",\"version\":\"1.2.0\",\"proto\":1,\"go\":\"go1.10.3\",\"host\":\"0.0.0.0\",\"port\":4222,\"max_payload\":1048576,\"client_id\":2392}\r\n";

    let protocol = inspector
        .check_server_initial_data(&config, 4222, DATA)
        .unwrap();
    assert_eq!(protocol, Protocol::Nats);
}
