/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
