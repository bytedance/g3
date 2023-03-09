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

use atoi::FromRadix10;

use super::IcapRespmodParseError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IcapRespmodResponsePayload {
    NoPayload,
    HttpResponseWithBody(usize),
    HttpResponseWithoutBody(usize),
}

impl IcapRespmodResponsePayload {
    pub(crate) fn parse(value: &str) -> Result<IcapRespmodResponsePayload, IcapRespmodParseError> {
        let mut parts = value.split(',');
        let hdr_part = parts
            .next()
            .ok_or(IcapRespmodParseError::InvalidHeaderValue("Encapsulated"))?
            .trim();

        let (name, value) = hdr_part
            .split_once('=')
            .ok_or(IcapRespmodParseError::InvalidHeaderValue("Encapsulated"))?;
        if value.ne("0") {
            return Err(IcapRespmodParseError::UnsupportedBody(
                "invalid hdr byte-offsets value".to_string(),
            ));
        }

        match name.to_lowercase().as_str() {
            "res-hdr" => {
                let body_part = parts
                    .next()
                    .ok_or_else(|| {
                        IcapRespmodParseError::UnsupportedBody(
                            "no body byte-offsets pair found".to_string(),
                        )
                    })?
                    .trim();
                let (name, value) = body_part.split_once('=').ok_or_else(|| {
                    IcapRespmodParseError::UnsupportedBody(
                        "invalid body byte-offsets pair".to_string(),
                    )
                })?;
                let (hdr_len, offset) = usize::from_radix_10(value.as_bytes());
                if offset != value.len() {
                    return Err(IcapRespmodParseError::UnsupportedBody(
                        "invalid body byte-offsets value".to_string(),
                    ));
                }
                match name.to_lowercase().as_str() {
                    "res-body" => Ok(IcapRespmodResponsePayload::HttpResponseWithBody(hdr_len)),
                    "null-body" => Ok(IcapRespmodResponsePayload::HttpResponseWithoutBody(hdr_len)),
                    _ => Err(IcapRespmodParseError::UnsupportedBody(
                        "invalid body byte-offsets name".to_string(),
                    )),
                }
            }
            "null-body" => Ok(IcapRespmodResponsePayload::NoPayload),
            _ => Err(IcapRespmodParseError::UnsupportedBody(
                "invalid hdr byte-offsets value".to_string(),
            )),
        }
    }
}
