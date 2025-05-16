/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod auth;
pub use auth::SocksAuthMethod;

mod error;
pub use error::{
    SocksConnectError, SocksNegotiationError, SocksReplyParseError, SocksRequestParseError,
    SocksUdpPacketError,
};

mod cmd;
pub use cmd::SocksCommand;

mod version;
pub use version::SocksVersion;
