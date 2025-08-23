/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
                "invalid hdr byte-offsets value",
            ));
        }

        match name.to_lowercase().as_str() {
            "res-hdr" => {
                let body_part = parts
                    .next()
                    .ok_or(IcapRespmodParseError::UnsupportedBody(
                        "no body byte-offsets pair found",
                    ))?
                    .trim();
                let (name, value) =
                    body_part
                        .split_once('=')
                        .ok_or(IcapRespmodParseError::UnsupportedBody(
                            "invalid body byte-offsets pair",
                        ))?;
                let (hdr_len, offset) = usize::from_radix_10(value.as_bytes());
                if offset != value.len() {
                    return Err(IcapRespmodParseError::UnsupportedBody(
                        "invalid body byte-offsets value",
                    ));
                }
                match name.to_lowercase().as_str() {
                    "res-body" => Ok(IcapRespmodResponsePayload::HttpResponseWithBody(hdr_len)),
                    "null-body" => Ok(IcapRespmodResponsePayload::HttpResponseWithoutBody(hdr_len)),
                    _ => Err(IcapRespmodParseError::UnsupportedBody(
                        "invalid body byte-offsets name",
                    )),
                }
            }
            "null-body" => Ok(IcapRespmodResponsePayload::NoPayload),
            _ => Err(IcapRespmodParseError::UnsupportedBody(
                "invalid hdr byte-offsets value",
            )),
        }
    }
}
