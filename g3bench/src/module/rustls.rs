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

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, value_parser};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use g3_types::net::{
    AlpnProtocol, RustlsCertificatePair, RustlsCertificatePairBuilder, RustlsClientConfig,
    RustlsClientConfigBuilder, UpstreamAddr,
};

const TLS_ARG_CA_CERT: &str = "tls-ca-cert";
const TLS_ARG_CERT: &str = "tls-cert";
const TLS_ARG_KEY: &str = "tls-key";
const TLS_ARG_NAME: &str = "tls-name";
const TLS_ARG_NO_SESSION_CACHE: &str = "tls-no-session-cache";
const TLS_ARG_NO_SNI: &str = "tls-no-sni";

const PROXY_TLS_ARG_CA_CERT: &str = "proxy-tls-ca-cert";
const PROXY_TLS_ARG_CERT: &str = "proxy-tls-cert";
const PROXY_TLS_ARG_KEY: &str = "proxy-tls-key";
const PROXY_TLS_ARG_NAME: &str = "proxy-tls-name";
const PROXY_TLS_ARG_NO_SESSION_CACHE: &str = "proxy-tls-no-session-cache";
const PROXY_TLS_ARG_NO_SNI: &str = "proxy-tls-no-sni";

pub(crate) trait AppendRustlsArgs {
    fn append_rustls_args(self) -> Self;
    #[allow(unused)]
    fn append_proxy_rustls_args(self) -> Self;
}

#[derive(Default)]
pub(crate) struct RustlsTlsClientArgs {
    pub(crate) config: Option<RustlsClientConfigBuilder>,
    pub(crate) client: Option<RustlsClientConfig>,
    pub(crate) tls_name: Option<ServerName<'static>>,
    pub(crate) cert_pair: Option<RustlsCertificatePair>,
    pub(crate) alpn_protocol: Option<AlpnProtocol>,
}

impl RustlsTlsClientArgs {
    pub(crate) async fn connect_target<S>(
        &self,
        tls_client: &RustlsClientConfig,
        stream: S,
        target: &UpstreamAddr,
    ) -> anyhow::Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_name = match &self.tls_name {
            Some(n) => n.clone(),
            None => ServerName::try_from(target.host())
                .map_err(|e| anyhow!("invalid tls server name P{}: {e}", target.host()))?,
        };
        let tls_connect = TlsConnector::from(tls_client.driver.clone()).connect(tls_name, stream);

        match tokio::time::timeout(tls_client.handshake_timeout, tls_connect).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => Err(anyhow!("failed to tls connect to peer: {e}")),
            Err(_) => Err(anyhow!("tls connect to peer timedout")),
        }
    }

    fn parse_tls_name(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        if let Some(name) = args.get_one::<String>(id) {
            let tls_name = ServerName::try_from(name.as_str())
                .map_err(|e| anyhow!("invalid tls server name {name}: {e}"))?;
            self.tls_name = Some(tls_name.to_owned());
        }
        Ok(())
    }

    fn parse_ca_cert(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if let Some(file) = args.get_one::<PathBuf>(id) {
            let ca_certs = load_certs(file).context(format!(
                "failed to load ca certs from file {}",
                file.display()
            ))?;
            tls_config.set_ca_certificates(ca_certs);
        }
        Ok(())
    }

    fn parse_client_auth(
        &mut self,
        args: &ArgMatches,
        cert_id: &str,
        key_id: &str,
    ) -> anyhow::Result<()> {
        let mut set_cert_pair = false;
        let mut cert_pair_builder = RustlsCertificatePairBuilder::default();
        if let Some(file) = args.get_one::<PathBuf>(cert_id) {
            let certs = load_certs(file).context(format!(
                "failed to load client certificate from file {}",
                file.display()
            ))?;
            cert_pair_builder.set_certs(certs);
            set_cert_pair = true;
        }
        if let Some(file) = args.get_one::<PathBuf>(key_id) {
            let key = load_key(file).context(format!(
                "failed to load client private key from file {}",
                file.display()
            ))?;
            cert_pair_builder.set_key(key);
            set_cert_pair = true;
        }
        if set_cert_pair {
            let cert_pair = cert_pair_builder
                .build()
                .context("failed to build client auth cert pair")?;
            self.cert_pair = Some(cert_pair);
        }
        Ok(())
    }

    fn parse_no_session_cache(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_no_session_cache();
        }
        Ok(())
    }

    fn parse_no_sni(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_disable_sni();
        }
        Ok(())
    }

    fn build_client(&mut self) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if let Some(cert_pair) = &self.cert_pair {
            tls_config.set_cert_pair(cert_pair.clone());
        }

        tls_config.check().context("invalid tls config")?;
        let tls_client = if let Some(p) = self.alpn_protocol {
            tls_config
                .build_with_alpn_protocols(Some(vec![p]))
                .context(format!("failed to build tls client with alpn protocol {p}"))?
        } else {
            tls_config.build().context("failed to build tls client")?
        };
        self.client = Some(tls_client);
        Ok(())
    }

    pub(crate) fn parse_tls_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if self.config.is_none() {
            return Ok(());
        }

        self.parse_tls_name(args, TLS_ARG_NAME)?;
        self.parse_ca_cert(args, TLS_ARG_CA_CERT)?;
        self.parse_client_auth(args, TLS_ARG_CERT, TLS_ARG_KEY)?;
        self.parse_no_session_cache(args, TLS_ARG_NO_SESSION_CACHE)?;
        self.parse_no_sni(args, TLS_ARG_NO_SNI)?;
        self.build_client()
    }

    #[allow(unused)]
    pub(crate) fn parse_proxy_tls_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if self.config.is_none() {
            return Ok(());
        }

        self.parse_tls_name(args, PROXY_TLS_ARG_NAME)?;
        self.parse_ca_cert(args, PROXY_TLS_ARG_CA_CERT)?;
        self.parse_client_auth(args, PROXY_TLS_ARG_CERT, PROXY_TLS_ARG_KEY)?;
        self.parse_no_session_cache(args, PROXY_TLS_ARG_NO_SESSION_CACHE)?;
        self.parse_no_sni(args, PROXY_TLS_ARG_NO_SNI)?;
        self.build_client()
    }
}

pub(crate) fn load_certs(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    let mut certs = Vec::new();
    for (i, cert) in CertificateDer::pem_reader_iter(file).enumerate() {
        let cert = cert
            .map_err(|e| anyhow!("invalid certificate #{i} in file {}: {e:?}", path.display()))?;
        certs.push(cert);
    }
    if certs.is_empty() {
        Err(anyhow!("no valid cert found in file {}", path.display()))
    } else {
        Ok(certs)
    }
}

pub(crate) fn load_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    PrivateKeyDer::from_pem_reader(file)
        .map_err(|e| anyhow!("invalid private key file {}: {e:?}", path.display()))
}

impl AppendRustlsArgs for Command {
    fn append_rustls_args(self) -> Command {
        append_tls_args(self)
    }

    fn append_proxy_rustls_args(self) -> Command {
        append_proxy_tls_args(self)
    }
}

pub(crate) fn append_tls_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new(TLS_ARG_NAME)
            .help("TLS verify name for target site")
            .value_name("SERVER NAME")
            .long(TLS_ARG_NAME)
            .num_args(1),
    )
    .arg(
        Arg::new(TLS_ARG_CA_CERT)
            .help("TLS CA certificate file for target site")
            .value_name("CA CERTIFICATE FILE")
            .long(TLS_ARG_CA_CERT)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf)),
    )
    .arg(
        Arg::new(TLS_ARG_CERT)
            .help("TLS client certificate file for target site")
            .value_name("CERTIFICATE FILE")
            .long(TLS_ARG_CERT)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf))
            .requires(TLS_ARG_KEY),
    )
    .arg(
        Arg::new(TLS_ARG_KEY)
            .help("TLS client private key file for target site")
            .value_name("PRIVATE KEY FILE")
            .long(TLS_ARG_KEY)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf))
            .requires(TLS_ARG_CERT),
    )
    .arg(
        Arg::new(TLS_ARG_NO_SESSION_CACHE)
            .help("Disable TLS session cache for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_NO_SESSION_CACHE),
    )
    .arg(
        Arg::new(TLS_ARG_NO_SNI)
            .help("Disable TLS SNI for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_NO_SNI),
    )
}

#[allow(unused)]
pub(crate) fn append_proxy_tls_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new(PROXY_TLS_ARG_NAME)
            .help("TLS verify name for proxy")
            .value_name("SERVER NAME")
            .long(PROXY_TLS_ARG_NAME)
            .num_args(1),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_CA_CERT)
            .help("TLS CA certificate file for proxy")
            .value_name("CA CERTIFICATE FILE")
            .long(PROXY_TLS_ARG_CA_CERT)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf)),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_CERT)
            .help("TLS client certificate file for proxy")
            .value_name("CERTIFICATE FILE")
            .long(PROXY_TLS_ARG_CERT)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf))
            .requires(PROXY_TLS_ARG_KEY),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_KEY)
            .help("TLS client private key file for proxy")
            .value_name("PRIVATE KEY FILE")
            .long(PROXY_TLS_ARG_KEY)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf))
            .requires(PROXY_TLS_ARG_CERT),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_NO_SESSION_CACHE)
            .help("Disable TLS session cache for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_NO_SESSION_CACHE),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_NO_SNI)
            .help("Disable TLS SNI for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_NO_SNI),
    )
}
