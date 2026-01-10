/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

mod id;
pub use id::{LdapMessageId, LdapMessageIdParseError};

mod length;
pub use length::{LdapMessageLength, LdapMessageLengthParseError};
