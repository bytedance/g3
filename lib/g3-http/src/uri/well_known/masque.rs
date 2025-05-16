/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
