/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;

use hickory_proto::error::ProtoError;
use quinn::{Connection, Endpoint, EndpointConfig, TokioRuntime};
use rustls::ClientConfig;

pub(crate) async fn quic_connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    mut tls_config: ClientConfig,
    tls_name: &str,
    alpn_protocol: &'static [u8],
) -> Result<Connection, ProtoError> {
    let bind_addr = bind_addr.unwrap_or_else(|| match name_server {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    });
    let sock = UdpSocket::bind(bind_addr)?;
    sock.connect(name_server)?;

    let endpoint_config = EndpointConfig::default(); // TODO set max payload size
    let mut endpoint = Endpoint::new(endpoint_config, None, sock, Arc::new(TokioRuntime))?;

    if tls_config.alpn_protocols.is_empty() {
        tls_config.alpn_protocols = vec![alpn_protocol.to_vec()];
    }
    let quinn_config = quinn::ClientConfig::new(Arc::new(tls_config));
    // TODO set transport config
    endpoint.set_default_client_config(quinn_config);

    let connection = endpoint
        .connect(name_server, tls_name)
        .map_err(|e| format!("quinn endpoint create error: {e}"))?
        .await
        .map_err(|e| format!("quinn endpoint connect error: {e}"))?;
    Ok(connection)
}
