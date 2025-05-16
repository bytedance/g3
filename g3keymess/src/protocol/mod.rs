/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

const MESSAGE_HEADER_LENGTH: usize = 8;
pub(crate) const MESSAGE_PADDED_LENGTH: usize = 1024;
const ITEM_HEADER_LENGTH: usize = 3;

mod request;
pub(crate) use request::{KeylessAction, KeylessRequest, KeylessRequestError};

mod response;
pub(crate) use response::{
    KeylessDataResponse, KeylessErrorResponse, KeylessPongResponse, KeylessResponse,
    KeylessResponseErrorCode,
};
