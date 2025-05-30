/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
