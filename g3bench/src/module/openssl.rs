/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, value_parser};
use openssl::pkey::{PKey, Private};
use openssl::ssl::SslVerifyMode;
use openssl::x509::X509;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_openssl::{SslConnector, SslStream};
use g3_types::net::{
    AlpnProtocol, Host, OpensslCertificatePair, OpensslClientConfig, OpensslClientConfigBuilder,
    OpensslProtocol, TlsVersion, UpstreamAddr,
};

const TLS_ARG_CA_CERT: &str = "tls-ca-cert";
const TLS_ARG_CERT: &str = "tls-cert";
const TLS_ARG_KEY: &str = "tls-key";
const TLS_ARG_NAME: &str = "tls-name";
const TLS_ARG_SESSION_CACHE: &str = "tls-session-cache";
const TLS_ARG_NO_VERIFY: &str = "tls-no-verify";
const TLS_ARG_NO_SNI: &str = "tls-no-sni";
const TLS_ARG_PROTOCOL: &str = "tls-protocol";
const TLS_ARG_VERSION_MIN: &str = "tls-version-min";
const TLS_ARG_VERSION_MAX: &str = "tls-version-max";
const TLS_ARG_CIPHERS: &str = "tls-ciphers";
const TLS_ARG_SUPPORTED_GROUPS: &str = "tls-supported-groups";
const TLS_ARG_USE_OCSP_STAPLING: &str = "tls-use-ocsp-stapling";
const TLS_ARG_ENABLE_SCT: &str = "tls-enable-sct";
const TLS_ARG_ENABLE_GREASE: &str = "tls-enable-grease";
const TLS_ARG_PERMUTE_EXTENSIONS: &str = "tls-permute-extensions";

const PROXY_TLS_ARG_CA_CERT: &str = "proxy-tls-ca-cert";
const PROXY_TLS_ARG_CERT: &str = "proxy-tls-cert";
const PROXY_TLS_ARG_KEY: &str = "proxy-tls-key";
const PROXY_TLS_ARG_NAME: &str = "proxy-tls-name";
const PROXY_TLS_ARG_SESSION_CACHE: &str = "proxy-tls-session-cache";
const PROXY_TLS_ARG_NO_VERIFY: &str = "proxy-tls-no-verify";
const PROXY_TLS_ARG_NO_SNI: &str = "proxy-tls-no-sni";
const PROXY_TLS_ARG_PROTOCOL: &str = "proxy-tls-protocol";
const PROXY_TLS_ARG_VERSION_MIN: &str = "proxy-tls-version-min";
const PROXY_TLS_ARG_VERSION_MAX: &str = "proxy-tls-version-max";
const PROXY_TLS_ARG_CIPHERS: &str = "proxy-tls-ciphers";
const PROXY_TLS_ARG_SUPPORTED_GROUPS: &str = "proxy-tls-supported-groups";
const PROXY_TLS_ARG_USE_OCSP_STAPLING: &str = "proxy-tls-use-ocsp-stapling";
const PROXY_TLS_ARG_ENABLE_SCT: &str = "proxy-tls-enable-sct";
const PROXY_TLS_ARG_ENABLE_GREASE: &str = "proxy-tls-enable-grease";
const PROXY_TLS_ARG_PERMUTE_EXTENSIONS: &str = "proxy-tls-permute-extensions";

const SESSION_CACHE_VALUES: [&str; 2] = ["off", "builtin"];
#[cfg(not(feature = "vendored-tongsuo"))]
const PROTOCOL_VALUES: [&str; 5] = ["ssl3.0", "tls1.0", "tls1.1", "tls1.2", "tls1.3"];
#[cfg(feature = "vendored-tongsuo")]
const PROTOCOL_VALUES: [&str; 6] = ["ssl3.0", "tls1.0", "tls1.1", "tls1.2", "tls1.3", "tlcp"];

pub(crate) trait AppendOpensslArgs {
    fn append_openssl_args(self) -> Self;
    fn append_proxy_openssl_args(self) -> Self;
}

#[derive(Default)]
pub(crate) struct OpensslTlsClientArgs {
    pub(crate) config: Option<OpensslClientConfigBuilder>,
    pub(crate) client: Option<OpensslClientConfig>,
    pub(crate) tls_name: Option<Host>,
    pub(crate) cert_pair: OpensslCertificatePair,
    pub(crate) no_verify: bool,
    pub(crate) alpn_protocol: Option<AlpnProtocol>,
}

impl OpensslTlsClientArgs {
    pub(crate) async fn connect_target<S>(
        &self,
        tls_client: &OpensslClientConfig,
        stream: S,
        target: &UpstreamAddr,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_name = self.tls_name.as_ref().unwrap_or_else(|| target.host());
        let mut ssl = tls_client
            .build_ssl(tls_name, target.port())
            .context("failed to build ssl context")?;
        if self.no_verify {
            ssl.set_verify(SslVerifyMode::NONE);
        }
        let tls_connector = SslConnector::new(ssl, stream)
            .map_err(|e| anyhow!("tls connector create failed: {e}"))?;
        let tls_stream = tls_connector
            .connect()
            .await
            .map_err(|e| anyhow!("tls connect to {tls_name} failed: {e}"))?;
        Ok(tls_stream)
    }

    fn parse_tls_name(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        if let Some(name) = args.get_one::<String>(id) {
            let host = Host::from_str(name).context(format!("invalid host name {name}"))?;
            self.tls_name = Some(host);
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
            tls_config
                .set_ca_certificates(ca_certs)
                .context("failed to set ca certificates")?;
        }
        Ok(())
    }

    fn parse_client_auth(
        &mut self,
        args: &ArgMatches,
        cert_id: &str,
        key_id: &str,
    ) -> anyhow::Result<()> {
        if let Some(file) = args.get_one::<PathBuf>(cert_id) {
            let cert = load_certs(file).context(format!(
                "failed to load client certificate from file {}",
                file.display()
            ))?;
            self.cert_pair
                .set_certificates(cert)
                .context("failed to set client certificate")?;
        }
        if let Some(file) = args.get_one::<PathBuf>(key_id) {
            let key = load_key(file).context(format!(
                "failed to load client private key from file {}",
                file.display()
            ))?;
            self.cert_pair
                .set_private_key(key)
                .context("failed to set client private key")?;
        }
        Ok(())
    }

    fn parse_protocol_and_args(
        &mut self,
        args: &ArgMatches,
        protocol_id: &str,
        ciphers_id: &str,
    ) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if let Some(protocol) = args.get_one::<String>(protocol_id) {
            let protocol =
                OpensslProtocol::from_str(protocol).context("invalid openssl protocol")?;
            tls_config.set_protocol(protocol);
        }
        if let Some(ciphers) = args.get_one::<String>(ciphers_id) {
            let ciphers = ciphers.split(':').map(|s| s.to_string()).collect();
            tls_config.set_ciphers(ciphers);
        }
        Ok(())
    }

    fn parse_tls_version(
        &mut self,
        args: &ArgMatches,
        version_min_id: &str,
        version_max_id: &str,
    ) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if let Some(version) = args.get_one::<TlsVersion>(version_min_id) {
            tls_config.set_min_tls_version(*version);
        }
        if let Some(version) = args.get_one::<TlsVersion>(version_max_id) {
            tls_config.set_max_tls_version(*version);
        }
        Ok(())
    }

    fn parse_session_cache(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        match args.get_one::<String>(id).map(|s| s.as_str()) {
            Some("off") => {
                tls_config.set_no_session_cache();
                Ok(())
            }
            Some("builtin") => {
                tls_config.set_use_builtin_session_cache();
                Ok(())
            }
            Some(s) => Err(anyhow!("unsupported session cache type {s}")),
            None => Ok(()),
        }
    }

    fn parse_no_verify(&mut self, args: &ArgMatches, id: &str) {
        if args.get_flag(id) {
            self.no_verify = true;
        }
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

    fn parse_supported_groups(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if let Some(groups) = args.get_one::<String>(id) {
            tls_config.set_supported_groups(groups.to_string());
        }
        Ok(())
    }

    fn parse_use_ocsp_stapling(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_use_ocsp_stapling(true);
        }
        Ok(())
    }

    fn parse_enable_sct(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_enable_sct(true);
        }
        Ok(())
    }

    fn parse_enable_grease(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_enable_grease(true);
        }
        Ok(())
    }

    fn parse_permute_extensions(&mut self, args: &ArgMatches, id: &str) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if args.get_flag(id) {
            tls_config.set_permute_extensions(true);
        }
        Ok(())
    }

    fn build_client(&mut self) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if self.cert_pair.is_set() {
            tls_config.set_cert_pair(self.cert_pair.clone());
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
        self.parse_protocol_and_args(args, TLS_ARG_PROTOCOL, TLS_ARG_CIPHERS)?;
        self.parse_tls_version(args, TLS_ARG_VERSION_MIN, TLS_ARG_VERSION_MAX)?;
        self.parse_session_cache(args, TLS_ARG_SESSION_CACHE)?;
        self.parse_no_verify(args, TLS_ARG_NO_VERIFY);
        self.parse_no_sni(args, TLS_ARG_NO_SNI)?;
        self.parse_supported_groups(args, TLS_ARG_SUPPORTED_GROUPS)?;
        self.parse_use_ocsp_stapling(args, TLS_ARG_USE_OCSP_STAPLING)?;
        self.parse_enable_sct(args, TLS_ARG_ENABLE_SCT)?;
        self.parse_enable_grease(args, TLS_ARG_ENABLE_GREASE)?;
        self.parse_permute_extensions(args, TLS_ARG_PERMUTE_EXTENSIONS)?;
        self.build_client()
    }

    pub(crate) fn parse_proxy_tls_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if self.config.is_none() {
            return Ok(());
        }

        self.parse_tls_name(args, PROXY_TLS_ARG_NAME)?;
        self.parse_ca_cert(args, PROXY_TLS_ARG_CA_CERT)?;
        self.parse_client_auth(args, PROXY_TLS_ARG_CERT, PROXY_TLS_ARG_KEY)?;
        self.parse_protocol_and_args(args, PROXY_TLS_ARG_PROTOCOL, PROXY_TLS_ARG_CIPHERS)?;
        self.parse_tls_version(args, PROXY_TLS_ARG_VERSION_MIN, PROXY_TLS_ARG_VERSION_MAX)?;
        self.parse_session_cache(args, PROXY_TLS_ARG_SESSION_CACHE)?;
        self.parse_no_verify(args, PROXY_TLS_ARG_NO_VERIFY);
        self.parse_no_sni(args, PROXY_TLS_ARG_NO_SNI)?;
        self.parse_supported_groups(args, PROXY_TLS_ARG_SUPPORTED_GROUPS)?;
        self.parse_use_ocsp_stapling(args, PROXY_TLS_ARG_USE_OCSP_STAPLING)?;
        self.parse_enable_sct(args, PROXY_TLS_ARG_ENABLE_SCT)?;
        self.parse_enable_grease(args, PROXY_TLS_ARG_ENABLE_GREASE)?;
        self.parse_permute_extensions(args, PROXY_TLS_ARG_PERMUTE_EXTENSIONS)?;
        self.build_client()
    }
}

pub(crate) fn load_certs(path: &Path) -> anyhow::Result<Vec<X509>> {
    const MAX_FILE_SIZE: usize = 4_000_000; // 4MB
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    let certs = X509::stack_from_pem(contents.as_bytes())
        .map_err(|e| anyhow!("invalid certificate file({}): {e}", path.display()))?;
    if certs.is_empty() {
        Err(anyhow!(
            "no valid certificate found in file {}",
            path.display()
        ))
    } else {
        Ok(certs)
    }
}

pub(crate) fn load_key(path: &Path) -> anyhow::Result<PKey<Private>> {
    const MAX_FILE_SIZE: usize = 256_000; // 256KB
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    PKey::private_key_from_pem(contents.as_bytes())
        .map_err(|e| anyhow!("invalid private key file({}): {e}", path.display()))
}

impl AppendOpensslArgs for Command {
    fn append_openssl_args(self) -> Command {
        append_tls_args(self)
    }

    fn append_proxy_openssl_args(self) -> Command {
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
        Arg::new(TLS_ARG_SESSION_CACHE)
            .help("Set TLS session cache type for target site")
            .value_name("TYPE")
            .long(TLS_ARG_SESSION_CACHE)
            .num_args(1)
            .value_parser(SESSION_CACHE_VALUES),
    )
    .arg(
        Arg::new(TLS_ARG_NO_VERIFY)
            .help("Skip TLS verify for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_NO_VERIFY),
    )
    .arg(
        Arg::new(TLS_ARG_NO_SNI)
            .help("Disable TLS SNI for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_NO_SNI),
    )
    .arg(
        Arg::new(TLS_ARG_SUPPORTED_GROUPS)
            .help("Set the supported elliptic curve groups for target site")
            .long(TLS_ARG_SUPPORTED_GROUPS)
            .num_args(1),
    )
    .arg(
        Arg::new(TLS_ARG_USE_OCSP_STAPLING)
            .help("Set to use OCSP stapling for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_USE_OCSP_STAPLING),
    )
    .arg(
        Arg::new(TLS_ARG_ENABLE_SCT)
            .help("Enable SCT for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_ENABLE_SCT),
    )
    .arg(
        Arg::new(TLS_ARG_ENABLE_GREASE)
            .help("Enable GREASE for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_ENABLE_GREASE),
    )
    .arg(
        Arg::new(TLS_ARG_PERMUTE_EXTENSIONS)
            .help("Permute TLS extensions for target site")
            .action(ArgAction::SetTrue)
            .long(TLS_ARG_PERMUTE_EXTENSIONS),
    )
    .arg(
        Arg::new(TLS_ARG_PROTOCOL)
            .help("Set tls protocol for target site")
            .value_name("PROTOCOL")
            .long(TLS_ARG_PROTOCOL)
            .value_parser(PROTOCOL_VALUES)
            .num_args(1),
    )
    .arg(
        Arg::new(TLS_ARG_CIPHERS)
            .help("Set tls ciphers for target site")
            .value_name("CIPHERS")
            .long(TLS_ARG_CIPHERS)
            .num_args(1)
            .requires(TLS_ARG_PROTOCOL),
    )
    .arg(
        Arg::new(TLS_ARG_VERSION_MIN)
            .help("Set minimum TLS version for target site")
            .conflicts_with(TLS_ARG_PROTOCOL)
            .value_name("TLS VERSION")
            .long(TLS_ARG_VERSION_MIN)
            .value_parser(value_parser!(TlsVersion))
            .num_args(1),
    )
    .arg(
        Arg::new(TLS_ARG_VERSION_MAX)
            .help("Set maximum TLS version for target site")
            .conflicts_with(TLS_ARG_PROTOCOL)
            .value_name("TLS VERSION")
            .long(TLS_ARG_VERSION_MAX)
            .value_parser(value_parser!(TlsVersion))
            .num_args(1),
    )
}

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
        Arg::new(PROXY_TLS_ARG_SESSION_CACHE)
            .help("Set TLS session cache type for proxy")
            .value_name("TYPE")
            .long(PROXY_TLS_ARG_SESSION_CACHE)
            .num_args(1)
            .value_parser(SESSION_CACHE_VALUES),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_NO_VERIFY)
            .help("Skip TLS verify for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_NO_VERIFY),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_NO_SNI)
            .help("Disable TLS SNI for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_NO_SNI),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_SUPPORTED_GROUPS)
            .help("Set the supported elliptic curve groups for proxy")
            .long(PROXY_TLS_ARG_SUPPORTED_GROUPS)
            .num_args(1),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_USE_OCSP_STAPLING)
            .help("Set to use OCSP stapling for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_USE_OCSP_STAPLING),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_ENABLE_SCT)
            .help("Enable SCT for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_ENABLE_SCT),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_ENABLE_GREASE)
            .help("Enable GREASE for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_ENABLE_GREASE),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_PERMUTE_EXTENSIONS)
            .help("Permute TLS extensions for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_PERMUTE_EXTENSIONS),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_PROTOCOL)
            .help("Set tls protocol for proxy")
            .value_name("PROTOCOL")
            .long(PROXY_TLS_ARG_PROTOCOL)
            .value_parser(PROTOCOL_VALUES)
            .num_args(1),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_CIPHERS)
            .help("Set tls ciphers for proxy")
            .value_name("CIPHERS")
            .long(PROXY_TLS_ARG_CIPHERS)
            .num_args(1)
            .requires(PROXY_TLS_ARG_PROTOCOL),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_VERSION_MIN)
            .help("Set minimum TLS version for proxy")
            .conflicts_with(PROXY_TLS_ARG_PROTOCOL)
            .value_name("TLS VERSION")
            .long(PROXY_TLS_ARG_VERSION_MIN)
            .value_parser(value_parser!(TlsVersion))
            .num_args(1),
    )
    .arg(
        Arg::new(PROXY_TLS_ARG_VERSION_MAX)
            .help("Set maximum TLS version for proxy")
            .conflicts_with(PROXY_TLS_ARG_PROTOCOL)
            .value_name("TLS VERSION")
            .long(PROXY_TLS_ARG_VERSION_MAX)
            .value_parser(value_parser!(TlsVersion))
            .num_args(1),
    )
}
