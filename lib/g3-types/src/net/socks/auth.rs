/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use url::Url;

use crate::auth::{AuthParseError, Password, Username};

#[derive(Clone, Eq, PartialEq)]
pub enum SocksAuth {
    None,
    User(Username, Password),
}

impl SocksAuth {
    pub fn code(&self) -> u8 {
        match self {
            SocksAuth::None => 0x00,
            SocksAuth::User(_, _) => 0x02,
        }
    }
}

impl TryFrom<&Url> for SocksAuth {
    type Error = AuthParseError;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let u = url.username();
        let auth = if u.is_empty() {
            SocksAuth::None
        } else {
            let username =
                Username::from_encoded(u).map_err(|_| AuthParseError::InvalidUsername)?;

            let password = if let Some(p) = url.password() {
                Password::from_encoded(p).map_err(|_| AuthParseError::InvalidPassword)?
            } else {
                return Err(AuthParseError::InvalidPassword);
            };

            SocksAuth::User(username, password)
        };

        Ok(auth)
    }
}
