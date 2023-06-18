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
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};
use rustls::{Certificate, PrivateKey};

use g3_types::net::{RustlsCertificatePair, RustlsClientConfig, RustlsClientConfigBuilder};

const TLS_ARG_CA_CERT: &str = "tls-ca-cert";
const TLS_ARG_CERT: &str = "tls-cert";
const TLS_ARG_KEY: &str = "tls-key";
const TLS_ARG_NAME: &str = "tls-name";
const TLS_ARG_NO_SESSION_CACHE: &str = "tls-no-session-cache";
const TLS_ARG_NO_VERIFY: &str = "tls-no-verify";
const TLS_ARG_NO_SNI: &str = "tls-no-sni";

const PROXY_TLS_ARG_CA_CERT: &str = "proxy-tls-ca-cert";
const PROXY_TLS_ARG_CERT: &str = "proxy-tls-cert";
const PROXY_TLS_ARG_KEY: &str = "proxy-tls-key";
const PROXY_TLS_ARG_NAME: &str = "proxy-tls-name";
const PROXY_TLS_ARG_NO_SESSION_CACHE: &str = "proxy-tls-no-session-cache";
const PROXY_TLS_ARG_NO_VERIFY: &str = "proxy-tls-no-verify";
const PROXY_TLS_ARG_NO_SNI: &str = "proxy-tls-no-sni";

pub(crate) trait AppendRustlsArgs {
    fn append_rustls_args(self) -> Self;
    fn append_proxy_rustls_args(self) -> Self;
}

#[derive(Default)]
pub(crate) struct RustlsTlsClientArgs {
    pub(crate) config: Option<RustlsClientConfigBuilder>,
    pub(crate) client: Option<RustlsClientConfig>,
    pub(crate) tls_name: Option<String>,
    pub(crate) cert_pair: RustlsCertificatePair,
    pub(crate) no_verify: bool,
}

impl RustlsTlsClientArgs {
    fn parse_tls_name(&mut self, args: &ArgMatches, id: &str) {
        if let Some(name) = args.get_one::<String>(id) {
            self.tls_name = Some(name.to_string());
        }
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
        if let Some(file) = args.get_one::<PathBuf>(cert_id) {
            let certs = load_certs(file).context(format!(
                "failed to load client certificate from file {}",
                file.display()
            ))?;
            self.cert_pair.certs = certs;
        }
        if let Some(file) = args.get_one::<PathBuf>(key_id) {
            let key = load_key(file).context(format!(
                "failed to load client private key from file {}",
                file.display()
            ))?;
            self.cert_pair.key = key;
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

    fn build_client(&mut self) -> anyhow::Result<()> {
        let tls_config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("no tls config found"))?;
        if self.cert_pair.is_set() {
            tls_config.set_cert_pair(self.cert_pair.clone());
        }

        tls_config.check().context("invalid tls config")?;
        let tls_client = tls_config.build().context("failed to build tls client")?;
        self.client = Some(tls_client);
        Ok(())
    }

    pub(crate) fn parse_tls_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if self.config.is_none() {
            return Ok(());
        }

        self.parse_tls_name(args, TLS_ARG_NAME);
        self.parse_ca_cert(args, TLS_ARG_CA_CERT)?;
        self.parse_client_auth(args, TLS_ARG_CERT, TLS_ARG_KEY)?;
        self.parse_no_session_cache(args, TLS_ARG_NO_SESSION_CACHE)?;
        self.parse_no_verify(args, TLS_ARG_NO_VERIFY);
        self.parse_no_sni(args, TLS_ARG_NO_SNI)?;
        self.build_client()
    }

    #[allow(unused)]
    pub(crate) fn parse_proxy_tls_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if self.config.is_none() {
            return Ok(());
        }

        self.parse_tls_name(args, PROXY_TLS_ARG_NAME);
        self.parse_ca_cert(args, PROXY_TLS_ARG_CA_CERT)?;
        self.parse_client_auth(args, PROXY_TLS_ARG_CERT, PROXY_TLS_ARG_KEY)?;
        self.parse_no_session_cache(args, PROXY_TLS_ARG_NO_SESSION_CACHE)?;
        self.parse_no_verify(args, PROXY_TLS_ARG_NO_VERIFY);
        self.parse_no_sni(args, PROXY_TLS_ARG_NO_SNI)?;
        self.build_client()
    }
}

pub(crate) fn load_certs(path: &Path) -> anyhow::Result<Vec<Certificate>> {
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    let certs = rustls_pemfile::certs(&mut BufReader::new(file))
        .map_err(|e| anyhow!("failed to read certs from file {}: {e}", path.display()))?;
    if certs.is_empty() {
        Err(anyhow!(
            "no valid certificate found in file {}",
            path.display()
        ))
    } else {
        Ok(certs.into_iter().map(Certificate).collect())
    }
}

pub(crate) fn load_key(path: &Path) -> anyhow::Result<PrivateKey> {
    use rustls_pemfile::Item;

    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    match rustls_pemfile::read_one(&mut BufReader::new(file)).map_err(|e| {
        anyhow!(
            "failed to read private key from file {}: {e}",
            path.display()
        )
    })? {
        Some(Item::PKCS8Key(d)) => Ok(PrivateKey(d)),
        Some(Item::RSAKey(d)) => Ok(PrivateKey(d)),
        Some(Item::ECKey(d)) => Ok(PrivateKey(d)),
        Some(item) => Err(anyhow!(
            "unsupported item in file {}: {item:?}",
            path.display()
        )),
        None => Err(anyhow!("no valid private key found")),
    }
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
        Arg::new(PROXY_TLS_ARG_NO_SESSION_CACHE)
            .help("Disable TLS session cache for proxy")
            .action(ArgAction::SetTrue)
            .long(PROXY_TLS_ARG_NO_SESSION_CACHE),
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
}
