/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod error;
pub(crate) use error::KeylessRecvMessageError;

mod header;
mod request;
mod response;

const KEYLESS_HEADER_LEN: usize = 8;

pub(crate) use header::KeylessHeader;
pub(crate) use request::KeylessRequest;
pub(crate) use response::{KeylessInternalErrorResponse, KeylessResponse, KeylessUpstreamResponse};
