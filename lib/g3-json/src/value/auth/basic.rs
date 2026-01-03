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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_username_ok() {
        // valid username
        let value = json!("valid_user");
        let username = as_username(&value).unwrap();
        assert_eq!(username.as_original(), "valid_user");

        // max length username (255 chars)
        let max_user = "a".repeat(255);
        let value = json!(max_user);
        let username = as_username(&value).unwrap();
        assert_eq!(username.len() as usize, 255);
    }

    #[test]
    fn as_username_err() {
        // non-string type
        let value = json!(123);
        assert!(as_username(&value).is_err());

        // username with colon
        let value = json!("invalid:user");
        assert!(as_username(&value).is_err());

        // username too long
        let long_user = "a".repeat(256);
        let value = json!(long_user);
        assert!(as_username(&value).is_err());
    }

    #[test]
    fn as_password_ok() {
        // valid password
        let value = json!("secure_password");
        let password = as_password(&value).unwrap();
        assert_eq!(password.as_original(), "secure_password");

        // max length password (255 chars)
        let max_pass = "b".repeat(255);
        let value = json!(max_pass);
        let password = as_password(&value).unwrap();
        assert_eq!(password.len() as usize, 255);
    }

    #[test]
    fn as_password_err() {
        // non-string type
        let value = json!(true);
        assert!(as_password(&value).is_err());

        // password too long
        let long_pass = "c".repeat(256);
        let value = json!(long_pass);
        assert!(as_password(&value).is_err());
    }
}
