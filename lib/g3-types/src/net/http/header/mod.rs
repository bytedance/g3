/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod map;
mod name;
mod value;

pub use map::HttpHeaderMap;
pub use name::HttpOriginalHeaderName;
pub use value::HttpHeaderValue;

mod forwarded;
mod server_id;

pub use forwarded::{
    HttpForwardedHeaderType, HttpForwardedHeaderValue, HttpStandardForwardedHeaderValue,
};
pub use server_id::HttpServerId;
