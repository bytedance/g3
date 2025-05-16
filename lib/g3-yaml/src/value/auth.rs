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
