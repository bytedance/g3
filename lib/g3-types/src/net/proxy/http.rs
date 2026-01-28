/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use url::Url;

use super::ProxyParseError;
use crate::net::{HttpAuth, UpstreamAddr};

#[cfg(feature = "openssl")]
use crate::net::OpensslClientConfigBuilder;

pub struct HttpProxy {
    peer: UpstreamAddr,
    pub auth: HttpAuth,
    #[cfg(feature = "openssl")]
    pub tls_config: Option<OpensslClientConfigBuilder>,
}

impl HttpProxy {
    pub fn peer(&self) -> &UpstreamAddr {
        &self.peer
    }

    pub(super) fn from_url_authority(url: &Url) -> Result<Self, ProxyParseError> {
        let host = url.host().ok_or(ProxyParseError::NoHostFound)?;
        let port = url.port().unwrap_or(8080);

        let peer = UpstreamAddr::new(host, port);

        let auth = HttpAuth::try_from(url)?;

        Ok(HttpProxy {
            peer,
            auth,
            #[cfg(feature = "openssl")]
            tls_config: None,
        })
    }

    #[cfg(feature = "openssl")]
    pub(super) fn from_url_authority_with_tls(url: &Url) -> Result<Self, ProxyParseError> {
        let mut v = HttpProxy::from_url_authority(url)?;
        let tls_config = OpensslClientConfigBuilder::with_cache_for_one_site();
        // for (_k, _v) in value.query_pairs() {
        //     // TODO set tls_config
        // }
        v.tls_config = Some(tls_config);
        Ok(v)
    }
}

impl TryFrom<&Url> for HttpProxy {
    type Error = ProxyParseError;

    fn try_from(value: &Url) -> Result<Self, Self::Error> {
        match value.scheme().to_ascii_lowercase().as_str() {
            "http" => HttpProxy::from_url_authority(value),
            #[cfg(feature = "openssl")]
            "https" => HttpProxy::from_url_authority_with_tls(value),
            _ => Err(ProxyParseError::InvalidScheme),
        }
    }
}
