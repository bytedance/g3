/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::net::SocketAddr;

use anyhow::{Context, anyhow};
use tokio::net::TcpStream;

use g3_io_ext::LimitedWriteExt;
use g3_io_ext::openssl::MaybeSslStream;
use g3_openssl::{SslConnector, SslStream};
use g3_socket::BindAddr;
use g3_types::net::{Host, OpensslClientConfig, UpstreamAddr};

use crate::auth::group::ldap::LdapMessageReceiver;
use crate::config::auth::LdapUserGroupConfig;

struct LdapTcpConnector {
    server: UpstreamAddr,
}

struct LdapTlsConnector {
    tcp_connector: LdapTcpConnector,
    tls_client: OpensslClientConfig,
}

pub(crate) enum LdapConnector {
    Tcp(LdapTcpConnector),
    Tls(LdapTlsConnector),
    StartTls(LdapTlsConnector),
}

impl LdapConnector {
    pub(crate) fn new(config: &LdapUserGroupConfig) -> anyhow::Result<Self> {
        let server = config.server.clone();
        let tcp_connector = LdapTcpConnector { server };
        let connector = match &config.tls_client {
            Some(builder) => {
                let tls_client = builder.build().context("failed to build tls client")?;
                let tls_connector = LdapTlsConnector {
                    tcp_connector,
                    tls_client,
                };
                if config.direct_tls {
                    LdapConnector::Tls(tls_connector)
                } else {
                    LdapConnector::StartTls(tls_connector)
                }
            }
            None => LdapConnector::Tcp(tcp_connector),
        };
        Ok(connector)
    }

    pub(crate) async fn connect(&self) -> anyhow::Result<MaybeSslStream<TcpStream>> {
        match self {
            LdapConnector::Tcp(c) => {
                let stream = c.connect().await?;
                Ok(MaybeSslStream::Plain(stream))
            }
            LdapConnector::Tls(c) => {
                let ssl_stream = c.direct_connect().await?;
                Ok(MaybeSslStream::Ssl(ssl_stream))
            }
            LdapConnector::StartTls(c) => {
                let ssl_stream = c.starttls_connect().await?;
                Ok(MaybeSslStream::Ssl(ssl_stream))
            }
        }
    }
}

impl LdapTcpConnector {
    async fn connect(&self) -> anyhow::Result<TcpStream> {
        let peer = match self.server.host() {
            Host::Ip(ip) => SocketAddr::new(*ip, self.server.port()),
            Host::Domain(domain) => {
                let addrs = tokio::net::lookup_host(format!("{domain}:{}", self.server.port()))
                    .await?
                    .collect::<Vec<_>>();
                let addr = fastrand::choice(addrs)
                    .ok_or_else(|| anyhow!("no address resolved for domain {domain}"))?;
                addr
            }
        };
        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            &BindAddr::None,
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| anyhow!("setup socket failed: {e}"))?;
        socket
            .connect(peer)
            .await
            .map_err(|e| anyhow!("can't connect to peer {peer}: {e}"))
    }
}

impl LdapTlsConnector {
    async fn direct_connect(&self) -> anyhow::Result<SslStream<TcpStream>> {
        let stream = self.tcp_connector.connect().await?;

        let ssl = self
            .tls_client
            .build_ssl(
                self.tcp_connector.server.host(),
                self.tcp_connector.server.port(),
            )
            .map_err(|e| anyhow!("build ssl context failed: {e}"))?;
        let tls_connector = SslConnector::new(ssl, stream)
            .map_err(|e| anyhow!("build ssl connector failed: {e}"))?;
        tls_connector
            .connect()
            .await
            .map_err(|e| anyhow!("tls connect failed: {e}"))
    }

    async fn starttls_connect(&self) -> anyhow::Result<SslStream<TcpStream>> {
        let mut stream = self.tcp_connector.connect().await?;

        let starttls_message: [u8; _] = [
            0x30, 0x1d, // Begin the LDAPMessage sequence
            0x02, 0x01, 0x7f, // The message ID (integer value 0x7f)
            0x77, 0x18, // Begin the extended request protocol op
            0x80, 0x16, // Begin the extended request OID "1.3.6.1.4.1.1466.20037"
            b'1', b'.', b'3', b'.', b'6', b'.', b'1', b'.', b'4', b'.', b'1', b'.', b'1', b'4',
            b'6', b'6', b'.', b'2', b'0', b'0', b'3', b'7',
        ];
        stream
            .write_all_flush(&starttls_message)
            .await
            .map_err(|e| anyhow!("failed to send StartTls extended request: {e}"))?;

        let mut rsp_receiver = LdapMessageReceiver::new(48);
        // TODO timeout
        let rsp = rsp_receiver
            .recv(&mut stream)
            .await
            .map_err(|e| anyhow!("failed to read StartTls response: {e}"))?;
        if !rsp.payload().ends_with(&starttls_message[7..]) {
            return Err(anyhow!(
                "unexpected StartTls response payload: {:?}",
                rsp.payload()
            ));
        }

        let ssl = self
            .tls_client
            .build_ssl(
                self.tcp_connector.server.host(),
                self.tcp_connector.server.port(),
            )
            .map_err(|e| anyhow!("build ssl context failed: {e}"))?;
        let tls_connector = SslConnector::new(ssl, stream)
            .map_err(|e| anyhow!("build ssl connector failed: {e}"))?;
        tls_connector
            .connect()
            .await
            .map_err(|e| anyhow!("tls connect failed: {e}"))
    }
}
