/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use uuid::{Timestamp, Uuid, v1::Context};

static UUID_CONTEXT: OnceLock<Context> = OnceLock::new();
static UUID_NODE_ID: OnceLock<[u8; 6]> = OnceLock::new();

pub fn generate_uuid(time: &DateTime<Utc>) -> Uuid {
    let context = UUID_CONTEXT.get_or_init(|| Context::new(rand::random()));
    let node_id = UUID_NODE_ID.get_or_init(|| {
        let mut bytes = [0u8; 6];
        rand::fill(&mut bytes);
        bytes
    });

    let ts = Timestamp::from_unix(
        context,
        time.timestamp() as u64,
        time.timestamp_subsec_nanos().min(999_999_999), // ignore leap second
    );
    Uuid::new_v1(ts, node_id)
}
