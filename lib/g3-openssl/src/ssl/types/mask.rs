/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use bitflags::bitflags;

bitflags! {
    pub struct SslInfoCallbackWhere: i32 {
        const LOOP = 0x01;
        const EXIT = 0x02;
        const READ = 0x04;
        const WRITE = 0x08;
        const HANDSHAKE_START = 0x10;
        const HANDSHAKE_DONE = 0x20;
        const ALERT = 0x4000;
    }
}
