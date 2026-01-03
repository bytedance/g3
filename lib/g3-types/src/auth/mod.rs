/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod error;
pub use error::{AuthParseError, UserAuthError};

mod user;
pub use user::{Password, Username};

#[cfg(feature = "auth-crypt")]
mod crypt;
#[cfg(feature = "auth-crypt")]
pub use crypt::FastHashedPassPhrase;

#[cfg(feature = "auth-facts")]
mod facts;
#[cfg(feature = "auth-facts")]
pub use facts::{FactsMatchType, FactsMatchValue};
