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

use smol_str::SmolStr;

use super::{WellKnownUri, WellKnownUriParser};
use crate::uri::{HttpMasque, UriParseError};

impl WellKnownUriParser<'_> {
    pub(super) fn parse_masque(&mut self) -> Result<WellKnownUri, UriParseError> {
        let Some(segment) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("segment"));
        };
        match segment {
            "udp" => {
                let Some(host) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target_host"));
                };

                let Some(port) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target_port"));
                };

                let masque = HttpMasque::new_udp(host, port)?;
                Ok(WellKnownUri::Masque(masque))
            }
            "ip" => {
                let Some(host) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target"));
                };

                let Some(proto) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("ipproto"));
                };

                let masque = HttpMasque::new_ip(host, proto)?;
                Ok(WellKnownUri::Masque(masque))
            }
            _ => Ok(WellKnownUri::Unsupported(SmolStr::from_iter([
                "masque", "/", segment,
            ]))),
        }
    }
}
