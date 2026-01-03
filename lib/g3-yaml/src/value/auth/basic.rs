/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::auth::{Password, Username};

pub fn as_username(value: &Yaml) -> anyhow::Result<Username> {
    if let Yaml::String(s) = value {
        Ok(Username::from_original(s)?)
    } else {
        Err(anyhow!("yaml value type for username should be string"))
    }
}

pub fn as_password(value: &Yaml) -> anyhow::Result<Password> {
    if let Yaml::String(s) = value {
        Ok(Password::from_original(s)?)
    } else {
        Err(anyhow!("yaml value type for password should be string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_username_ok() {
        // valid username
        let yaml = yaml_str!("test_user");
        assert_eq!(as_username(&yaml).unwrap().as_original(), "test_user");

        // valid maximum length (255)
        let max_len_username = "a".repeat(255);
        let yaml = Yaml::String(max_len_username.to_string());
        assert_eq!(as_username(&yaml).unwrap().as_original(), max_len_username);
    }

    #[test]
    fn as_username_err() {
        // non-string type (integer)
        let yaml = Yaml::Integer(123);
        assert!(as_username(&yaml).is_err());

        // colon in username
        let yaml = yaml_str!("invalid:username");
        assert!(as_username(&yaml).is_err());

        // exceeding maximum length (256)
        let too_long_username = "a".repeat(256);
        let yaml = Yaml::String(too_long_username.to_string());
        assert!(as_username(&yaml).is_err());
    }

    #[test]
    fn as_password_ok() {
        // valid password
        let yaml = yaml_str!("secure_password123");
        assert_eq!(
            as_password(&yaml).unwrap().as_original(),
            "secure_password123"
        );

        // maximum length (255)
        let max_len_password = "b".repeat(255);
        let yaml = Yaml::String(max_len_password.to_string());
        assert_eq!(as_password(&yaml).unwrap().as_original(), max_len_password);
    }

    #[test]
    fn as_password_err() {
        // non-string type (boolean)
        let yaml = Yaml::Boolean(true);
        assert!(as_password(&yaml).is_err());

        // exceeding maximum length (256)
        let too_long_password = "b".repeat(256);
        let yaml = Yaml::String(too_long_password.to_string());
        assert!(as_password(&yaml).is_err());
    }
}
