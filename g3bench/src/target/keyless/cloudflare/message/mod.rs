/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod request;
pub(crate) use request::{KeylessRequest, KeylessRequestBuilder};

mod response;
pub(crate) use response::{KeylessLocalError, KeylessResponse, KeylessResponseError};

const MESSAGE_HEADER_LENGTH: usize = 8;
const MESSAGE_PADDED_LENGTH: usize = 1024;
const ITEM_HEADER_LENGTH: usize = 3;
