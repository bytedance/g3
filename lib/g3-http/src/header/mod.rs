/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod auth;
pub use auth::{proxy_authenticate_basic, proxy_authorization_basic, www_authenticate_basic};

mod connection;
pub use connection::{Connection, connection_as_bytes};

mod content;
pub use content::{content_length, content_range_overflowed, content_range_sized, content_type};

mod transfer;
pub use transfer::transfer_encoding_chunked;
