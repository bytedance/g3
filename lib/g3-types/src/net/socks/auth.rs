/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
