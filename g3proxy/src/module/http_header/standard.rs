/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use base64::prelude::*;

pub(crate) fn proxy_authorization_basic_pass(userid: &str) -> String {
    format!(
        "Proxy-Authorization: Basic {}\r\n",
        BASE64_STANDARD.encode(format!("{userid}:{}", crate::build::PKG_NAME))
    )
}
