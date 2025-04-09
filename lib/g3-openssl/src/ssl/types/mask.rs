/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
