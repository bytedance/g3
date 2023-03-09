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

use thiserror::Error;
use url::Url;

use crate::auth::AuthParseError;
use crate::net::UpstreamAddr;

mod common;
pub use common::ProxyRequestType;

mod http;
pub use self::http::HttpProxy;

mod socks4;
pub use socks4::Socks4Proxy;

mod socks5;
pub use socks5::Socks5Proxy;

#[derive(Debug, Error)]
pub enum ProxyParseError {
    #[error("invalid scheme")]
    InvalidScheme,
    #[error("no host found")]
    NoHostFound,
    #[error("auth parse failed: {0}")]
    InvalidAuth(#[from] AuthParseError),
    #[error("invalid tls config: {0}")]
    InvalidTlsConfig(anyhow::Error),
}

pub enum Proxy {
    Http(HttpProxy),
    Socks4(Socks4Proxy),
    Socks5(Socks5Proxy),
}

impl Proxy {
    pub fn peer(&self) -> &UpstreamAddr {
        match self {
            Proxy::Http(p) => p.peer(),
            Proxy::Socks4(p) => p.peer(),
            Proxy::Socks5(p) => p.peer(),
        }
    }
}

impl TryFrom<&Url> for Proxy {
    type Error = ProxyParseError;

    fn try_from(value: &Url) -> Result<Self, Self::Error> {
        match value.scheme().to_ascii_lowercase().as_str() {
            "http" => {
                let p = HttpProxy::from_url_authority(value)?;
                Ok(Proxy::Http(p))
            }
            "https" => {
                let p = HttpProxy::from_url_authority_with_tls(value)?;
                Ok(Proxy::Http(p))
            }
            "socks4" | "socks4a" => {
                let p = Socks4Proxy::from_url_authority(value)?;
                Ok(Proxy::Socks4(p))
            }
            "socks5" | "socks5h" => {
                let p = Socks5Proxy::from_url_authority(value)?;
                Ok(Proxy::Socks5(p))
            }
            _ => Err(ProxyParseError::InvalidScheme),
        }
    }
}
