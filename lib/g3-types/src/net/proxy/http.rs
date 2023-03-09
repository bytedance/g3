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

use super::ProxyParseError;
use crate::net::{HttpAuth, UpstreamAddr};

use crate::net::OpensslTlsClientConfigBuilder;

pub struct HttpProxy {
    peer: UpstreamAddr,
    pub auth: HttpAuth,
    pub tls_config: Option<OpensslTlsClientConfigBuilder>,
}

impl HttpProxy {
    pub fn peer(&self) -> &UpstreamAddr {
        &self.peer
    }

    pub(super) fn from_url_authority(url: &Url) -> Result<Self, ProxyParseError> {
        let host = url.host().ok_or(ProxyParseError::NoHostFound)?;
        let port = url.port().unwrap_or(8080);

        let peer = UpstreamAddr::from_url_host_and_port(host.to_owned(), port);

        let auth = HttpAuth::try_from(url)?;

        Ok(HttpProxy {
            peer,
            auth,
            tls_config: None,
        })
    }

    pub(super) fn from_url_authority_with_tls(url: &Url) -> Result<Self, ProxyParseError> {
        let mut v = HttpProxy::from_url_authority(url)?;
        let tls_config = OpensslTlsClientConfigBuilder::with_cache_for_one_site();
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
            "https" => HttpProxy::from_url_authority_with_tls(value),
            _ => Err(ProxyParseError::InvalidScheme),
        }
    }
}
