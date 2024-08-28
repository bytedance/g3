/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
