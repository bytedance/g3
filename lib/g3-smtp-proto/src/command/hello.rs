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

use g3_types::net::Host;

use super::CommandLineError;

pub(super) fn parse_host(msg: &[u8]) -> Result<Host, CommandLineError> {
    let host_b = match memchr::memchr(b' ', msg) {
        Some(p) => &msg[..p],
        None => msg,
    };
    Host::parse_smtp_host_address(host_b).ok_or(CommandLineError::InvalidClientHost)
}
