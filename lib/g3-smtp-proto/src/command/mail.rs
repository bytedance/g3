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

use std::str;

use super::CommandLineError;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MailParam {
    reverse_path: String,
}

impl MailParam {
    #[inline]
    pub fn reverse_path(&self) -> &str {
        &self.reverse_path
    }

    pub(super) fn parse(msg: &[u8]) -> Result<Self, CommandLineError> {
        let msg = str::from_utf8(msg).map_err(CommandLineError::InvalidUtf8Command)?;

        let mut iter = msg.split_ascii_whitespace();
        let s = iter.next().ok_or(CommandLineError::InvalidCommandParam(
            "MAIL",
            "no reverse path present",
        ))?;

        let reverse_path = s
            .to_lowercase()
            .strip_prefix("from:")
            .map(|s| s.to_string())
            .ok_or(CommandLineError::InvalidCommandParam(
                "MAIL",
                "invalid reverse path prefix",
            ))?;

        Ok(MailParam { reverse_path })
    }
}
