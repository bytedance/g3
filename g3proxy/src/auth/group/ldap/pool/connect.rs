/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use tokio::net::TcpStream;

use g3_codec::ldap::{LdapResult, LdapSequence};
use g3_io_ext::LimitedWriteExt;
use g3_io_ext::openssl::MaybeSslStream;
use g3_openssl::{SslConnector, SslStream};
use g3_socket::BindAddr;
use g3_types::net::{Host, OpensslClientConfig};

use crate::auth::group::ldap::LdapMessageReceiver;
use crate::config::auth::LdapUserGroupConfig;

pub(super) struct LdapTcpConnector {
    config: Arc<LdapUserGroupConfig>,
}

pub(super) struct LdapTlsConnector {
    config: Arc<LdapUserGroupConfig>,
    tls_client: OpensslClientConfig,
}

pub(super) enum LdapConnector {
    Tcp(LdapTcpConnector),
    Tls(LdapTlsConnector),
    StartTls(LdapTlsConnector),
}

impl LdapConnector {
    pub(super) fn new(config: Arc<LdapUserGroupConfig>) -> anyhow::Result<Self> {
        let connector = match &config.tls_client {
            Some(builder) => {
                let tls_client = builder.build().context("failed to build tls client")?;
                if config.direct_tls {
                    LdapConnector::Tls(LdapTlsConnector { config, tls_client })
                } else {
                    LdapConnector::StartTls(LdapTlsConnector { config, tls_client })
                }
            }
            None => LdapConnector::Tcp(LdapTcpConnector { config }),
        };
        Ok(connector)
    }

    pub(super) async fn connect(&self) -> anyhow::Result<MaybeSslStream<TcpStream>> {
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
        let peer = match self.config.server.host() {
            Host::Ip(ip) => SocketAddr::new(*ip, self.config.server.port()),
            Host::Domain(domain) => {
                let addrs =
                    tokio::net::lookup_host(format!("{domain}:{}", self.config.server.port()))
                        .await?
                        .collect::<Vec<_>>();
                fastrand::choice(addrs)
                    .ok_or_else(|| anyhow!("no address resolved for domain {domain}"))?
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
        tokio::time::timeout(self.config.connect_timeout, socket.connect(peer))
            .await
            .map_err(|_| anyhow!("timed out connecting to peer {peer}"))?
            .map_err(|e| anyhow!("can't connect to peer {peer}: {e}"))
    }
}

impl LdapTlsConnector {
    async fn direct_connect(&self) -> anyhow::Result<SslStream<TcpStream>> {
        let stream = self.tcp_connect().await?;
        self.tls_handshake(stream).await
    }

    async fn starttls_connect(&self) -> anyhow::Result<SslStream<TcpStream>> {
        let mut stream = self.tcp_connect().await?;
        self.starttls(&mut stream).await?;
        self.tls_handshake(stream).await
    }

    async fn tcp_connect(&self) -> anyhow::Result<TcpStream> {
        let tcp_connector = LdapTcpConnector {
            config: self.config.clone(),
        };
        tcp_connector.connect().await
    }

    async fn tls_handshake(&self, stream: TcpStream) -> anyhow::Result<SslStream<TcpStream>> {
        let tls_name = self
            .config
            .tls_name
            .as_ref()
            .unwrap_or_else(|| self.config.server.host());
        let ssl = self
            .tls_client
            .build_ssl(tls_name, self.config.server.port())
            .map_err(|e| anyhow!("build ssl context failed: {e}"))?;
        let tls_connector = SslConnector::new(ssl, stream)
            .map_err(|e| anyhow!("build ssl connector failed: {e}"))?;
        tokio::time::timeout(self.tls_client.handshake_timeout, tls_connector.connect())
            .await
            .map_err(|_| anyhow!("tls handshake with peer {} timed out", self.config.server))?
            .map_err(|e| anyhow!("tls connect failed: {e}"))
    }

    async fn starttls(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
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

        let rsp = tokio::time::timeout(self.config.response_timeout, rsp_receiver.recv(stream))
            .await
            .map_err(|_| anyhow!("timed out when waiting for STARTTLS response"))?
            .map_err(|e| anyhow!("failed to read StartTls response: {e}"))?;

        let rsp_sequence = LdapSequence::parse_extended_response(rsp.payload())
            .map_err(|e| anyhow!("invalid ldap response sequence: {e}"))?;
        let data = rsp_sequence.data();
        let result =
            LdapResult::parse(data).map_err(|e| anyhow!("invalid ldap response result: {e}"))?;
        if !result.is_success() {
            return Err(anyhow!(
                "STARTTLS failed with error code {}: {}",
                result.result_code(),
                result.diagnostic_message()
            ));
        }

        let ext_data = &data[result.encoded_len()..];
        if ext_data.is_empty() {
            // OID is optional
            return Ok(());
        }

        let oid = LdapSequence::parse_extended_response_oid(ext_data)
            .map_err(|e| anyhow!("invalid ldap extended response oid sequence: {e}"))?;
        if oid.data() == b"1.3.6.1.4.1.1466.20037" {
            Err(anyhow!(
                "unexpected StartTls response payload: {:?}",
                rsp.payload()
            ))
        } else {
            Ok(())
        }
    }
}
