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

use std::sync::Arc;

use hickory_proto::ProtoError;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{Connection, Endpoint, EndpointConfig, TokioRuntime};
use rustls::ClientConfig;

use g3_socket::UdpConnectInfo;

pub(crate) async fn quic_connect(
    connect_info: UdpConnectInfo,
    mut tls_config: ClientConfig,
    tls_name: &str,
    alpn_protocol: &'static [u8],
) -> Result<Connection, ProtoError> {
    let sock = connect_info.udp_connect()?;

    let endpoint_config = EndpointConfig::default(); // TODO set max payload size
    let mut endpoint = Endpoint::new(endpoint_config, None, sock, Arc::new(TokioRuntime))?;

    if tls_config.alpn_protocols.is_empty() {
        tls_config.alpn_protocols = vec![alpn_protocol.to_vec()];
    }
    let quic_config = QuicClientConfig::try_from(tls_config)
        .map_err(|e| format!("invalid quic tls config: {e}"))?;
    let client_config = quinn::ClientConfig::new(Arc::new(quic_config));
    // TODO set transport config
    endpoint.set_default_client_config(client_config);

    let connection = endpoint
        .connect(connect_info.server, tls_name)
        .map_err(|e| format!("quinn endpoint create error: {e}"))?
        .await
        .map_err(|e| format!("quinn endpoint connect error: {e}"))?;
    Ok(connection)
}
