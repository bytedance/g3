/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, value_parser};
use hickory_client::client::Client;
use hickory_proto::BufDnsStreamHandle;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;

use g3_types::net::{DnsEncryptionProtocol, RustlsClientConfigBuilder};

use super::{DnsRequest, DnsRequestPickState};
use crate::module::rustls::{AppendRustlsArgs, RustlsTlsClientArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const DNS_ARG_TARGET: &str = "target";
const DNS_ARG_TIMEOUT: &str = "timeout";
const DNS_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const DNS_ARG_ENCRYPTION: &str = "encryption";
const DNS_ARG_TCP: &str = "tcp";
const DNS_ARG_INPUT: &str = "input";
const DNS_ARG_QUERY_REQUESTS: &str = "query-requests";
const DNS_ARG_DUMP_RESULT: &str = "dump-result";
const DNS_ARG_ITER_GLOBAL: &str = "iter-global";

#[cfg(feature = "quic")]
const DNS_ENCRYPTION_PROTOCOLS: [&str; 4] = ["dot", "doh", "doh3", "doq"];
#[cfg(not(feature = "quic"))]
const DNS_ENCRYPTION_PROTOCOLS: [&str; 2] = ["dot", "doh"];

#[derive(Default)]
pub(super) struct GlobalRequestPicker {
    id: AtomicUsize,
}

impl DnsRequestPickState for GlobalRequestPicker {
    fn pick_next(&self, max: usize) -> usize {
        let mut id = self.id.load(Ordering::Acquire);
        loop {
            let mut next = id + 1;
            if next > max {
                next = 0;
            }

            match self
                .id
                .compare_exchange(id, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return id,
                Err(n) => id = n,
            }
        }
    }
}

pub(super) struct BenchDnsArgs {
    target: SocketAddr,
    encryption: Option<DnsEncryptionProtocol>,
    use_tcp: bool,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    socket: SocketArgs,
    tls: RustlsTlsClientArgs,

    requests: Vec<DnsRequest>,
    pub(super) dump_result: bool,
    pub(super) iter_global: bool,
    pub(super) global_picker: GlobalRequestPicker,
}

impl BenchDnsArgs {
    fn new(target: SocketAddr) -> Self {
        let tls = RustlsTlsClientArgs {
            config: Some(RustlsClientConfigBuilder::default()),
            ..Default::default()
        };
        BenchDnsArgs {
            target,
            encryption: None,
            use_tcp: false,
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(10),
            socket: SocketArgs::default(),
            tls,
            requests: Vec::new(),
            dump_result: false,
            iter_global: false,
            global_picker: GlobalRequestPicker::default(),
        }
    }

    pub(super) fn fetch_request<P: DnsRequestPickState>(&self, pick: &P) -> Option<&DnsRequest> {
        match self.requests.len() {
            0 => None,
            1 => Some(&self.requests[0]),
            n => {
                let next = pick.pick_next(n - 1);
                self.requests.get(next)
            }
        }
    }

    pub(super) async fn new_dns_client(&self) -> anyhow::Result<Client> {
        if let Some(p) = self.encryption {
            let tls_client = self
                .tls
                .client
                .as_ref()
                .ok_or_else(|| anyhow!("no valid tls client config found"))?;

            match p {
                DnsEncryptionProtocol::Tls => {
                    self.new_dns_over_tls_client(tls_client.driver.as_ref().clone())
                        .await
                }
                DnsEncryptionProtocol::Https => {
                    self.new_dns_over_h2_client(tls_client.driver.as_ref().clone())
                        .await
                }
                #[cfg(feature = "quic")]
                DnsEncryptionProtocol::H3 => {
                    self.new_dns_over_h3_client(tls_client.driver.as_ref().clone())
                        .await
                }
                #[cfg(feature = "quic")]
                DnsEncryptionProtocol::Quic => {
                    self.new_dns_over_quic_client(tls_client.driver.as_ref().clone())
                        .await
                }
            }
        } else if self.use_tcp {
            self.new_dns_over_tcp_client().await
        } else {
            self.new_dns_over_udp_client().await
        }
    }

    async fn new_dns_over_udp_client(&self) -> anyhow::Result<Client> {
        // FIXME should we use random port?
        let connect_info = self.socket.hickory_udp_connect_info(self.target);
        let client_connect = g3_hickory_client::io::udp::connect(connect_info, self.timeout);

        let (client, bg) = Client::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tcp_client(&self) -> anyhow::Result<Client> {
        let (message_sender, outbound_messages) = BufDnsStreamHandle::new(self.target);

        let connect_info = self.socket.hickory_tcp_connect_info(self.target);
        let tcp_connect = g3_hickory_client::io::tcp::connect(
            connect_info,
            outbound_messages,
            self.connect_timeout,
        );

        let (client, bg) =
            Client::with_timeout(Box::pin(tcp_connect), message_sender, self.timeout, None)
                .await
                .map_err(|e| anyhow!("failed to create tcp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tls_client(&self, tls_client: ClientConfig) -> anyhow::Result<Client> {
        let (message_sender, outbound_messages) = BufDnsStreamHandle::new(self.target);

        let tls_name = self
            .tls
            .tls_name
            .clone()
            .unwrap_or_else(|| ServerName::IpAddress(self.target.ip().into()));
        let connect_info = self.socket.hickory_tcp_connect_info(self.target);
        let tls_connect = g3_hickory_client::io::tls::connect(
            connect_info,
            tls_client,
            tls_name,
            outbound_messages,
            self.connect_timeout,
        );

        let (client, bg) =
            Client::with_timeout(Box::pin(tls_connect), message_sender, self.timeout, None)
                .await
                .map_err(|e| anyhow!("failed to create tls async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_h2_client(&self, tls_client: ClientConfig) -> anyhow::Result<Client> {
        let tls_name = self
            .tls
            .tls_name
            .clone()
            .unwrap_or_else(|| ServerName::IpAddress(self.target.ip().into()));

        let connect_info = self.socket.hickory_tcp_connect_info(self.target);
        let client_connect = g3_hickory_client::io::h2::connect(
            connect_info,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.timeout,
        );

        let (client, bg) = Client::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create h2 async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    #[cfg(feature = "quic")]
    async fn new_dns_over_h3_client(&self, tls_client: ClientConfig) -> anyhow::Result<Client> {
        let tls_name = match &self.tls.tls_name {
            Some(ServerName::DnsName(domain)) => domain.as_ref().to_string(),
            Some(ServerName::IpAddress(ip)) => IpAddr::from(*ip).to_string(),
            Some(_) => return Err(anyhow!("unsupported tls server name type")),
            None => self.target.ip().to_string(),
        };

        let connect_info = self.socket.hickory_udp_connect_info(self.target);
        let client_connect = g3_hickory_client::io::h3::connect(
            connect_info,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.timeout,
        );

        let (client, bg) = Client::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create h3 async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    #[cfg(feature = "quic")]
    async fn new_dns_over_quic_client(&self, tls_client: ClientConfig) -> anyhow::Result<Client> {
        let tls_name = match &self.tls.tls_name {
            Some(ServerName::DnsName(domain)) => domain.as_ref().to_string(),
            Some(ServerName::IpAddress(ip)) => IpAddr::from(*ip).to_string(),
            Some(_) => return Err(anyhow!("unsupported tls server name type")),
            None => self.target.ip().to_string(),
        };

        let connect_info = self.socket.hickory_udp_connect_info(self.target);
        let client_connect = g3_hickory_client::io::quic::connect(
            connect_info,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.timeout,
        );

        let (client, bg) = Client::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }
}

pub(super) fn add_dns_args(app: Command) -> Command {
    app.arg(
        Arg::new(DNS_ARG_TARGET)
            .help("Target dns server address (default port will be used if missing)")
            .required(true)
            .num_args(1),
    )
    .arg(
        Arg::new(DNS_ARG_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("DNS query timeout")
            .default_value("10s")
            .long(DNS_ARG_TIMEOUT)
            .num_args(1),
    )
    .arg(
        Arg::new(DNS_ARG_CONNECT_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("Timeout for connection to next peer")
            .default_value("10s")
            .long(DNS_ARG_CONNECT_TIMEOUT)
            .num_args(1),
    )
    .arg(
        Arg::new(DNS_ARG_ENCRYPTION)
            .value_name("PROTOCOL")
            .help("Use encrypted dns protocol")
            .long(DNS_ARG_ENCRYPTION)
            .short('e')
            .num_args(1)
            .value_parser(DNS_ENCRYPTION_PROTOCOLS)
            .conflicts_with(DNS_ARG_TCP),
    )
    .arg(
        Arg::new(DNS_ARG_TCP)
            .help("Use tcp instead of udp")
            .action(ArgAction::SetTrue)
            .long(DNS_ARG_TCP)
            .conflicts_with(DNS_ARG_ENCRYPTION),
    )
    .arg(
        Arg::new(DNS_ARG_QUERY_REQUESTS)
            .help(
                "requests to query.\n\
                    in the form <DOMAIN> or <DOMAIN>,<RTYPE> or <DOMAIN>,<CLASS>,<RTYPE>",
            )
            .conflicts_with(DNS_ARG_INPUT),
    )
    .arg(
        Arg::new(DNS_ARG_INPUT)
            .help("input file that contains the requests, one per line")
            .num_args(1)
            .long(DNS_ARG_INPUT)
            .value_parser(value_parser!(PathBuf))
            .value_hint(ValueHint::FilePath)
            .conflicts_with(DNS_ARG_QUERY_REQUESTS),
    )
    .arg(
        Arg::new(DNS_ARG_DUMP_RESULT)
            .help("Dump the query answer")
            .action(ArgAction::SetTrue)
            .long(DNS_ARG_DUMP_RESULT),
    )
    .arg(
        Arg::new(DNS_ARG_ITER_GLOBAL)
            .help("Iter requests globally")
            .action(ArgAction::SetTrue)
            .long(DNS_ARG_ITER_GLOBAL),
    )
    .append_socket_args()
    .append_rustls_args()
}

pub(super) fn parse_dns_args(args: &ArgMatches) -> anyhow::Result<BenchDnsArgs> {
    let Some(target) = args.get_one::<String>(DNS_ARG_TARGET) else {
        return Err(anyhow!("no target set"));
    };
    let mut dns_args = if let Ok(addr) = SocketAddr::from_str(target) {
        BenchDnsArgs::new(addr)
    } else if let Ok(ip) = IpAddr::from_str(target) {
        BenchDnsArgs::new(SocketAddr::new(ip, 0))
    } else {
        return Err(anyhow!("invalid dns server address {target}"));
    };

    if let Some(timeout) = g3_clap::humanize::get_duration(args, DNS_ARG_TIMEOUT)? {
        dns_args.timeout = timeout;
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, DNS_ARG_CONNECT_TIMEOUT)? {
        dns_args.connect_timeout = timeout;
    }

    if args.get_flag(DNS_ARG_TCP) {
        dns_args.use_tcp = true;
    }
    if let Some(s) = args.get_one::<String>(DNS_ARG_ENCRYPTION) {
        let p = DnsEncryptionProtocol::from_str(s).context("invalid dns encryption protocol")?;
        dns_args.encryption = Some(p);
    }
    if dns_args.target.port() == 0 {
        let default_port = dns_args.encryption.map(|e| e.default_port()).unwrap_or(53);
        dns_args.target.set_port(default_port);
    }

    if let Some(requests) = args.get_many::<String>(DNS_ARG_QUERY_REQUESTS) {
        for r in requests {
            let req = DnsRequest::parse_one(r)?;
            dns_args.requests.push(req);
        }
    } else if let Some(p) = args.get_one::<PathBuf>(DNS_ARG_INPUT) {
        let f =
            File::open(p).map_err(|e| anyhow!("failed to open input file {}: {e}", p.display()))?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            match line {
                Ok(s) => {
                    let req = DnsRequest::parse_one(&s)?;
                    dns_args.requests.push(req);
                }
                Err(e) => return Err(anyhow!("failed to read next line: {e}")),
            }
        }
    }

    if args.get_flag(DNS_ARG_DUMP_RESULT) {
        dns_args.dump_result = true;
    }
    if args.get_flag(DNS_ARG_ITER_GLOBAL) {
        dns_args.iter_global = true;
    }

    dns_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;
    dns_args
        .tls
        .parse_tls_args(args)
        .context("invalid tls config")?;

    Ok(dns_args)
}
