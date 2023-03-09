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
use crate::net::UpstreamAddr;

pub struct Socks4Proxy {
    peer: UpstreamAddr,
}

impl Socks4Proxy {
    pub fn peer(&self) -> &UpstreamAddr {
        &self.peer
    }

    pub(super) fn from_url_authority(url: &Url) -> Result<Self, ProxyParseError> {
        let host = url.host().ok_or(ProxyParseError::NoHostFound)?;
        let port = url.port().unwrap_or(1080);

        let peer = UpstreamAddr::from_url_host_and_port(host.to_owned(), port);

        Ok(Socks4Proxy { peer })
    }
}
