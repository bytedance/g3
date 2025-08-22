/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::auth::FastHashedPassPhrase;
use g3_xcrypt::XCryptHash;

mod json;
mod yaml;

const CONFIG_KEY_TYPE: &str = "type";

#[derive(Clone)]
pub(crate) enum PasswordToken {
    Forbidden,
    SkipVerify,
    FastHash(FastHashedPassPhrase),
    XCrypt(XCryptHash),
}
