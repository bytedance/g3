/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use base64::prelude::*;

use g3_types::auth::{Password, Username};

pub fn proxy_authorization_basic(username: &Username, password: &Password) -> String {
    format!(
        "Proxy-Authorization: Basic {}\r\n",
        BASE64_STANDARD.encode(format!(
            "{}:{}",
            username.as_original(),
            password.as_original()
        ))
    )
}

pub fn proxy_authenticate_basic(realm: &str) -> String {
    format!("Proxy-Authenticate: Basic realm=\"{realm}\"\r\n")
}

pub fn www_authenticate_basic(realm: &str) -> String {
    format!("WWW-Authenticate: Basic realm=\"{realm}\"\r\n")
}
