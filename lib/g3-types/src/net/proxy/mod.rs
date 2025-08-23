/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;
use url::Url;

use crate::auth::AuthParseError;
use crate::net::UpstreamAddr;

mod common;
pub use common::ProxyRequestType;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "http")]
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

#[allow(clippy::large_enum_variant)]
pub enum Proxy {
    #[cfg(feature = "http")]
    Http(HttpProxy),
    Socks4(Socks4Proxy),
    Socks5(Socks5Proxy),
}

impl Proxy {
    pub fn peer(&self) -> &UpstreamAddr {
        match self {
            #[cfg(feature = "http")]
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
            #[cfg(feature = "http")]
            "http" => {
                let p = HttpProxy::from_url_authority(value)?;
                Ok(Proxy::Http(p))
            }
            #[cfg(all(feature = "http", feature = "openssl"))]
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
