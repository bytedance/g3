/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod error;
mod user;

pub use error::{AuthParseError, UserAuthError};
pub use user::{Password, Username};

#[cfg(feature = "auth-crypt")]
mod crypt;

#[cfg(feature = "auth-crypt")]
pub use crypt::FastHashedPassPhrase;
