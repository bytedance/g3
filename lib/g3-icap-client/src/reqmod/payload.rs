/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use atoi::FromRadix10;

use super::IcapReqmodParseError;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum IcapReqmodResponsePayload {
    NoPayload,
    HttpRequestWithBody(usize),
    HttpRequestWithoutBody(usize),
    HttpResponseWithBody(usize),
    HttpResponseWithoutBody(usize),
}

impl IcapReqmodResponsePayload {
    pub(crate) fn parse(value: &str) -> Result<IcapReqmodResponsePayload, IcapReqmodParseError> {
        let mut parts = value.split(',');
        let hdr_part = parts
            .next()
            .ok_or(IcapReqmodParseError::InvalidHeaderValue("Encapsulated"))?
            .trim();

        let (name, value) = hdr_part
            .split_once('=')
            .ok_or(IcapReqmodParseError::InvalidHeaderValue("Encapsulated"))?;
        if value.ne("0") {
            return Err(IcapReqmodParseError::UnsupportedBody(
                "invalid hdr byte-offsets value",
            ));
        }

        match name.to_lowercase().as_str() {
            "req-hdr" => {
                let body_part = parts
                    .next()
                    .ok_or(IcapReqmodParseError::UnsupportedBody(
                        "no body byte-offsets pair found",
                    ))?
                    .trim();
                let (name, value) =
                    body_part
                        .split_once('=')
                        .ok_or(IcapReqmodParseError::UnsupportedBody(
                            "invalid body byte-offsets pair",
                        ))?;
                let (hdr_len, offset) = usize::from_radix_10(value.as_bytes());
                if offset != value.len() {
                    return Err(IcapReqmodParseError::UnsupportedBody(
                        "invalid body byte-offsets value",
                    ));
                }
                match name.to_lowercase().as_str() {
                    "req-body" => Ok(IcapReqmodResponsePayload::HttpRequestWithBody(hdr_len)),
                    "null-body" => Ok(IcapReqmodResponsePayload::HttpRequestWithoutBody(hdr_len)),
                    _ => Err(IcapReqmodParseError::UnsupportedBody(
                        "invalid body byte-offsets name",
                    )),
                }
            }
            "res-hdr" => {
                let body_part = parts
                    .next()
                    .ok_or(IcapReqmodParseError::UnsupportedBody(
                        "no body byte-offsets pair found",
                    ))?
                    .trim();
                let (name, value) =
                    body_part
                        .split_once('=')
                        .ok_or(IcapReqmodParseError::UnsupportedBody(
                            "invalid body byte-offsets pair",
                        ))?;
                let (hdr_len, offset) = usize::from_radix_10(value.as_bytes());
                if offset != value.len() {
                    return Err(IcapReqmodParseError::UnsupportedBody(
                        "invalid body byte-offsets value",
                    ));
                }
                match name.to_lowercase().as_str() {
                    "res-body" => Ok(IcapReqmodResponsePayload::HttpResponseWithBody(hdr_len)),
                    "null-body" => Ok(IcapReqmodResponsePayload::HttpResponseWithoutBody(hdr_len)),
                    _ => Err(IcapReqmodParseError::UnsupportedBody(
                        "invalid body byte-offsets name",
                    )),
                }
            }
            "null-body" => Ok(IcapReqmodResponsePayload::NoPayload),
            _ => Err(IcapReqmodParseError::UnsupportedBody(
                "invalid hdr byte-offsets value",
            )),
        }
    }
}
