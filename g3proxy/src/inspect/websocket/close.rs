/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

pub struct ServerCloseFrame {}

impl ServerCloseFrame {
    pub(super) const fn encode_with_status_code(status_code: u16) -> [u8; 4] {
        let code = status_code.to_be_bytes();
        [0x88, 0x02, code[0], code[1]]
    }
}

pub struct ClientCloseFrame {}

impl ClientCloseFrame {
    pub(super) const fn encode_with_status_code(status_code: u16) -> [u8; 8] {
        let code = status_code.to_be_bytes();
        [0x88, 0x82, 0x00, 0x00, 0x00, 0x00, code[0], code[1]]
    }
}
