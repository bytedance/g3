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
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};
use rustls::ClientConfig;
use tokio::net::{TcpStream, UdpSocket};
use trust_dns_client::client::AsyncClient;
use trust_dns_proto::iocompat::AsyncIoTokioAsStd;

use g3_types::net::{DnsEncryptionProtocol, RustlsClientConfigBuilder};

use super::{DnsRequest, DnsRequestPickState};
use crate::target::{AppendRustlsArgs, RustlsTlsClientArgs};

const DNS_ARG_TARGET: &str = "target";
const DNS_ARG_LOCAL_ADDRESS: &str = "local-address";
const DNS_ARG_TIMEOUT: &str = "timeout";
const DNS_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const DNS_ARG_ENCRYPTION: &str = "encryption";
const DNS_ARG_TCP: &str = "tcp";
const DNS_ARG_INPUT: &str = "input";
const DNS_ARG_QUERY_REQUESTS: &str = "query-requests";
const DNS_ARG_DUMP_RESULT: &str = "dump-result";
const DNS_ARG_ITER_GLOBAL: &str = "iter-global";

const DNS_ENCRYPTION_PROTOCOLS: [&str; 3] = ["dot", "doh", "doq"];

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
    bind: Option<SocketAddr>,
    encryption: Option<DnsEncryptionProtocol>,
    use_tcp: bool,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,
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
            bind: None,
            encryption: None,
            use_tcp: false,
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(10),
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

    pub(super) async fn new_dns_client(&self) -> anyhow::Result<AsyncClient> {
        if let Some(p) = self.encryption {
            let tls_client = self.tls.client.as_ref().ok_or_else(|| anyhow!(""))?;
            let tls_name = self
                .tls
                .tls_name
                .clone()
                .unwrap_or_else(|| self.target.ip().to_string());

            match p {
                DnsEncryptionProtocol::Tls => {
                    self.new_dns_over_tls_client(tls_client.driver.clone(), tls_name)
                        .await
                }
                DnsEncryptionProtocol::Https => {
                    self.new_dns_over_https_client(tls_client.driver.clone(), tls_name)
                        .await
                }
                DnsEncryptionProtocol::Quic => {
                    self.new_dns_over_quic_client(tls_client.driver.as_ref(), tls_name)
                        .await
                }
            }
        } else if self.use_tcp {
            self.new_dns_over_tcp_client().await
        } else {
            self.new_dns_over_udp_client().await
        }
    }

    async fn new_dns_over_udp_client(&self) -> anyhow::Result<AsyncClient> {
        // FIXME should we use random port?
        let client_connect =
            trust_dns_client::udp::UdpClientStream::<UdpSocket>::with_bind_addr_and_timeout(
                self.target,
                self.bind,
                self.connect_timeout,
            );

        let (client, bg) = AsyncClient::connect(client_connect)
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tcp_client(&self) -> anyhow::Result<AsyncClient> {
        let (stream, sender) =
            trust_dns_client::tcp::TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::with_bind_addr_and_timeout(
                self.target,
                self.bind,
                self.connect_timeout,
            );

        let (client, bg) = AsyncClient::new(stream, sender, None)
            .await
            .map_err(|e| anyhow!("failed to create tcp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tls_client(
        &self,
        tls_client: Arc<ClientConfig>,
        tls_name: String,
    ) -> anyhow::Result<AsyncClient> {
        let (stream, sender) = trust_dns_proto::rustls::tls_client_connect_with_bind_addr::<
            AsyncIoTokioAsStd<TcpStream>,
        >(self.target, self.bind, tls_name, tls_client);

        let (client, bg) = AsyncClient::new(stream, sender, None)
            .await
            .map_err(|e| anyhow!("failed to create tls async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_https_client(
        &self,
        tls_client: Arc<ClientConfig>,
        tls_name: String,
    ) -> anyhow::Result<AsyncClient> {
        let mut builder =
            trust_dns_proto::https::HttpsClientStreamBuilder::with_client_config(tls_client);
        if let Some(addr) = self.bind {
            builder.bind_addr(addr);
        }
        let client_connect = builder.build::<AsyncIoTokioAsStd<TcpStream>>(self.target, tls_name);

        let (client, bg) = AsyncClient::connect(client_connect)
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_quic_client(
        &self,
        tls_client: &ClientConfig,
        tls_name: String,
    ) -> anyhow::Result<AsyncClient> {
        let mut builder = trust_dns_proto::quic::QuicClientStream::builder();
        builder.crypto_config(tls_client.clone());
        if let Some(addr) = self.bind {
            builder.bind_addr(addr);
        }
        let client_connect = builder.build(self.target, tls_name);

        let (client, bg) = AsyncClient::connect(client_connect)
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
        Arg::new(DNS_ARG_LOCAL_ADDRESS)
            .value_name("LOCAL SOCKET ADDRESS")
            .short('B')
            .long(DNS_ARG_LOCAL_ADDRESS)
            .num_args(1)
            .value_parser(value_parser!(IpAddr)),
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
            .num_args(0)
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

    if let Some(ip) = args.get_one::<SocketAddr>(DNS_ARG_LOCAL_ADDRESS) {
        dns_args.bind = Some(*ip);
    }

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
        .tls
        .parse_tls_args(args)
        .context("invalid tls config")?;

    Ok(dns_args)
}
