/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use rand::rngs::OsRng;
use uuid::{v1::Context, Timestamp, Uuid};

static UUID_CONTEXT: OnceLock<Context> = OnceLock::new();
static UUID_NODE_ID: OnceLock<[u8; 6]> = OnceLock::new();

pub fn generate_uuid(time: &DateTime<Utc>) -> Uuid {
    let context = UUID_CONTEXT.get_or_init(|| {
        use rand::Rng;

        Context::new(OsRng.gen())
    });
    let node_id = UUID_NODE_ID.get_or_init(|| {
        use rand::RngCore;

        let mut bytes = [0u8; 6];
        OsRng.fill_bytes(&mut bytes);
        bytes
    });

    let ts = Timestamp::from_unix(
        context,
        time.timestamp() as u64,
        time.timestamp_subsec_nanos().max(999_999_999), // ignore leap second
    );
    Uuid::new_v1(ts, node_id)
}
