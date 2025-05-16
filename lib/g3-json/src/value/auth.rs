/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use serde_json::Value;

use g3_types::auth::{Password, Username};

pub fn as_username(value: &Value) -> anyhow::Result<Username> {
    if let Value::String(s) = value {
        Ok(Username::from_original(s)?)
    } else {
        Err(anyhow!("json value type for username should be string"))
    }
}

pub fn as_password(value: &Value) -> anyhow::Result<Password> {
    if let Value::String(s) = value {
        Ok(Password::from_original(s)?)
    } else {
        Err(anyhow!("json value type for password should be string"))
    }
}
