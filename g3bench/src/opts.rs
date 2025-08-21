/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::io::{IsTerminal, stderr};
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use ahash::AHashMap;
use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, value_parser};

use g3_runtime::blended::BlendedRuntimeConfig;
use g3_runtime::unaided::UnaidedRuntimeConfig;
use g3_statsd_client::{StatsdBackend, StatsdClient, StatsdClientConfig};
use g3_types::collection::{SelectivePickPolicy, SelectiveVec, SelectiveVecBuilder, WeightedValue};
use g3_types::limit::RateLimitQuota;
use g3_types::metrics::NodeName;
use g3_types::net::{TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig, UpstreamAddr};

use super::progress::BenchProgress;

const GLOBAL_ARG_UNAIDED: &str = "unaided";
const GLOBAL_ARG_UNCONSTRAINED: &str = "unconstrained";
const GLOBAL_ARG_THREADS: &str = "threads";
const GLOBAL_ARG_THREAD_STACK_SIZE: &str = "thread-stack-size";
const GLOBAL_ARG_OPENSSL_ASYNC_JOB_INIT_SIZE: &str = "openssl-async-job-init-size";
const GLOBAL_ARG_OPENSSL_ASYNC_JOB_MAX_SIZE: &str = "openssl-async-job-max-size";
const GLOBAL_ARG_CONCURRENCY: &str = "concurrency";
const GLOBAL_ARG_LATENCY: &str = "latency";
const GLOBAL_ARG_TIME_LIMIT: &str = "time-limit";
const GLOBAL_ARG_RATE_LIMIT: &str = "rate-limit";
const GLOBAL_ARG_REQUESTS: &str = "requests";
const GLOBAL_ARG_RESOLVE: &str = "resolve";
const GLOBAL_ARG_LOG_ERROR: &str = "log-error";
const GLOBAL_ARG_IGNORE_FATAL_ERROR: &str = "ignore-fatal-error";
const GLOBAL_ARG_EMIT_METRICS: &str = "emit-metrics";
const GLOBAL_ARG_STATSD_TARGET_UDP: &str = "statsd-target-udp";
const GLOBAL_ARG_STATSD_TARGET_UNIX: &str = "statsd-target-unix";
const GLOBAL_ARG_NO_PROGRESS_BAR: &str = "no-progress-bar";
const GLOBAL_ARG_NO_SUMMARY: &str = "no-summary";

const GLOBAL_ARG_PEER_PICK_POLICY: &str = "peer-pick-policy";
const GLOBAL_ARG_TCP_LIMIT_SHIFT: &str = "tcp-limit-shift";
const GLOBAL_ARG_TCP_LIMIT_BYTES: &str = "tcp-limit-bytes";
const GLOBAL_ARG_UDP_LIMIT_SHIFT: &str = "udp-limit-shift";
const GLOBAL_ARG_UDP_LIMIT_BYTES: &str = "udp-limit-bytes";
const GLOBAL_ARG_UDP_LIMIT_PACKETS: &str = "udp-limit-packets";

pub struct ProcArgs {
    pub(super) concurrency: NonZeroUsize,
    pub(super) latency: Option<Duration>,
    pub(super) requests: Option<usize>,
    pub(super) time_limit: Option<Duration>,
    pub(super) rate_limit: Option<RateLimitQuota>,
    pub(super) log_error_count: usize,
    pub(super) ignore_fatal_error: bool,
    pub(super) task_unconstrained: bool,
    resolver: AHashMap<UpstreamAddr, IpAddr>,
    pub(super) use_unaided_worker: bool,
    worker_runtime: UnaidedRuntimeConfig,
    main_runtime: BlendedRuntimeConfig,

    statsd_client_config: Option<StatsdClientConfig>,
    no_progress_bar: bool,
    pub(super) no_summary: bool,

    peer_pick_policy: SelectivePickPolicy,
    pub(super) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(super) udp_sock_speed_limit: UdpSockSpeedLimitConfig,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            concurrency: NonZeroUsize::MIN,
            latency: None,
            requests: None,
            time_limit: None,
            rate_limit: None,
            log_error_count: 0,
            ignore_fatal_error: false,
            task_unconstrained: false,
            resolver: AHashMap::new(),
            use_unaided_worker: false,
            worker_runtime: UnaidedRuntimeConfig::default(),
            main_runtime: BlendedRuntimeConfig::default(),
            statsd_client_config: None,
            no_progress_bar: false,
            no_summary: false,
            peer_pick_policy: SelectivePickPolicy::RoundRobin,
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
            udp_sock_speed_limit: UdpSockSpeedLimitConfig::default(),
        }
    }
}

impl ProcArgs {
    pub fn summary(&self) {
        if self.no_summary {
            return;
        }

        println!("Concurrency Level: {}", self.concurrency);
        println!();
    }

    pub(super) fn new_progress_bar(&self) -> Option<BenchProgress> {
        if self.no_progress_bar {
            None
        } else if let Some(requests) = self.requests {
            Some(BenchProgress::new_fixed(requests))
        } else {
            self.time_limit.map(BenchProgress::new_timed)
        }
    }

    pub(super) fn new_statsd_client(&self) -> Option<(StatsdClient, Duration)> {
        if let Some(config) = &self.statsd_client_config {
            match config.build() {
                Ok(client) => {
                    let pid = std::process::id();
                    let mut buffer = itoa::Buffer::new();
                    let client = client.with_tag("pid", buffer.format(pid));
                    Some((client, config.emit_interval))
                }
                Err(e) => {
                    eprintln!("unable to build statsd client: {e}");
                    None
                }
            }
        } else {
            None
        }
    }

    fn parse_resolve_value(&mut self, v: &str) -> anyhow::Result<()> {
        let mut parts = v.rsplitn(2, ':');

        let ip = parts
            .next()
            .ok_or_else(|| anyhow!("no upstream field found"))?;
        let upstream = parts.next().ok_or_else(|| anyhow!("no ip field found"))?;

        let upstream = UpstreamAddr::from_str(upstream).context("invalid upstream addr")?;
        let ip = IpAddr::from_str(ip).map_err(|e| anyhow!("invalid ip address: {e}"))?;

        self.resolver.insert(upstream, ip);
        Ok(())
    }

    pub(super) async fn resolve(
        &self,
        upstream: &UpstreamAddr,
    ) -> anyhow::Result<SelectiveVec<WeightedValue<SocketAddr>>> {
        let mut builder = SelectiveVecBuilder::new();
        if let Some(ip) = self.resolver.get(upstream) {
            let addr = SocketAddr::new(*ip, upstream.port());
            builder.insert(WeightedValue::new(addr));
        } else {
            let addrs = tokio::net::lookup_host(upstream.to_string())
                .await
                .map_err(|e| anyhow!("failed to resolve address for {upstream}: {e:?}"))?;
            for addr in addrs {
                builder.insert(WeightedValue::new(addr));
            }
        }
        builder
            .build()
            .ok_or_else(|| anyhow!("no resolved address"))
    }

    pub(super) fn select_peer<'a, T: Hash>(
        &self,
        peers: &'a SelectiveVec<WeightedValue<T>>,
    ) -> &'a T {
        match self.peer_pick_policy {
            SelectivePickPolicy::Random => peers.pick_random().inner(),
            SelectivePickPolicy::Serial => peers.pick_serial().inner(),
            SelectivePickPolicy::RoundRobin => peers.pick_round_robin().inner(),
            _ => unreachable!(),
        }
    }

    pub fn main_runtime(&self) -> BlendedRuntimeConfig {
        if self.use_unaided_worker {
            let mut main_runtime = BlendedRuntimeConfig::new();
            main_runtime.set_thread_number(0);
            main_runtime
        } else {
            self.main_runtime.clone()
        }
    }

    pub fn worker_runtime(&self) -> Option<&UnaidedRuntimeConfig> {
        if self.use_unaided_worker {
            Some(&self.worker_runtime)
        } else {
            None
        }
    }
}

pub fn add_global_args(app: Command) -> Command {
    app.arg(
        Arg::new(GLOBAL_ARG_CONCURRENCY)
            .help("Number of multiple requests to make at a time")
            .value_name("CONCURRENCY COUNT")
            .short('c')
            .long(GLOBAL_ARG_CONCURRENCY)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize))
            .default_value("1"),
    )
    .arg(
        Arg::new(GLOBAL_ARG_LATENCY)
            .help("Latency between serial tasks in milliseconds")
            .value_name("LATENCY TIME")
            .short('l')
            .long(GLOBAL_ARG_LATENCY)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_TIME_LIMIT)
            .help("Maximum time to spend for benchmarking")
            .value_name("TOTAL TIME")
            .global(true)
            .short('t')
            .long(GLOBAL_ARG_TIME_LIMIT)
            .num_args(1),
    )
    .arg(
        Arg::new(GLOBAL_ARG_RATE_LIMIT)
            .help("Maximum request rate limit")
            .value_name("RATE LIMIT")
            .global(true)
            .short('r')
            .long(GLOBAL_ARG_RATE_LIMIT)
            .num_args(1),
    )
    .arg(
        Arg::new(GLOBAL_ARG_REQUESTS)
            .help("Number of requests to perform")
            .value_name("REQUEST COUNT")
            .global(true)
            .short('n')
            .long(GLOBAL_ARG_REQUESTS)
            .num_args(1)
            .value_parser(value_parser!(usize)),
        // FIXME use default_value and default_value_if(GLOBAL_ARG_TIME_LIMIT, None, None)
        //       after these methods support global args
    )
    .arg(
        Arg::new(GLOBAL_ARG_RESOLVE)
            .help("Provide a custom address for a specific host and port pair")
            .value_name("host:port:addr")
            .global(true)
            .long(GLOBAL_ARG_RESOLVE)
            .action(ArgAction::Append),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UNAIDED)
            .help("Use unaided worker for tasks")
            .global(true)
            .long(GLOBAL_ARG_UNAIDED)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UNCONSTRAINED)
            .help("Run benchmark task unconstrained")
            .global(true)
            .long(GLOBAL_ARG_UNCONSTRAINED)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(GLOBAL_ARG_THREADS)
            .help("Number of threads")
            .value_name("THREAD NUMBER")
            .long(GLOBAL_ARG_THREADS)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(NonZeroUsize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_THREAD_STACK_SIZE)
            .long(GLOBAL_ARG_THREAD_STACK_SIZE)
            .value_name("STACK SIZE")
            .global(true)
            .num_args(1),
    )
    .arg(
        Arg::new(GLOBAL_ARG_OPENSSL_ASYNC_JOB_INIT_SIZE)
            .help("Set OpenSSL async job init size")
            .value_name("INIT SIZE")
            .global(true)
            .long(GLOBAL_ARG_OPENSSL_ASYNC_JOB_INIT_SIZE)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_OPENSSL_ASYNC_JOB_MAX_SIZE)
            .help("Set OpenSSL async job max size")
            .value_name("MAX SIZE")
            .global(true)
            .long(GLOBAL_ARG_OPENSSL_ASYNC_JOB_MAX_SIZE)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_LOG_ERROR)
            .help("Number of error requests to log")
            .value_name("COUNT")
            .long(GLOBAL_ARG_LOG_ERROR)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_IGNORE_FATAL_ERROR)
            .help("Continue even if fatal error occurred")
            .long(GLOBAL_ARG_IGNORE_FATAL_ERROR)
            .global(true)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(GLOBAL_ARG_EMIT_METRICS)
            .help("Set if we need to emit metrics to statsd")
            .action(ArgAction::SetTrue)
            .long(GLOBAL_ARG_EMIT_METRICS)
            .global(true),
    )
    .arg(
        Arg::new(GLOBAL_ARG_STATSD_TARGET_UDP)
            .help("Set the udp statsd target address")
            .value_name("UDP SOCKET ADDRESS")
            .long(GLOBAL_ARG_STATSD_TARGET_UDP)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(SocketAddr)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_STATSD_TARGET_UNIX)
            .help("Set the unix statsd target address")
            .value_name("UNIX SOCKET ADDRESS")
            .long(GLOBAL_ARG_STATSD_TARGET_UNIX)
            .global(true)
            .num_args(1)
            .value_hint(ValueHint::FilePath)
            .value_parser(value_parser!(PathBuf)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_NO_PROGRESS_BAR)
            .help("Disable progress bar")
            .action(ArgAction::SetTrue)
            .long(GLOBAL_ARG_NO_PROGRESS_BAR)
            .global(true),
    )
    .arg(
        Arg::new(GLOBAL_ARG_NO_SUMMARY)
            .help("Disable summary output")
            .action(ArgAction::SetTrue)
            .long(GLOBAL_ARG_NO_SUMMARY)
            .global(true),
    )
    .arg(
        Arg::new(GLOBAL_ARG_PEER_PICK_POLICY)
            .help("Set the pick policy for selecting peers")
            .long(GLOBAL_ARG_PEER_PICK_POLICY)
            .global(true)
            .value_parser(["rr", "random", "serial"])
            .default_value("rr")
            .num_args(1),
    )
    .arg(
        Arg::new(GLOBAL_ARG_TCP_LIMIT_SHIFT)
            .help("Shift value for the TCP socket speed limit config")
            .value_name("SHIFT VALUE")
            .long(GLOBAL_ARG_TCP_LIMIT_SHIFT)
            .global(true)
            .num_args(1)
            .value_parser(["2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"])
            .default_value("10")
            .requires(GLOBAL_ARG_TCP_LIMIT_BYTES),
    )
    .arg(
        Arg::new(GLOBAL_ARG_TCP_LIMIT_BYTES)
            .help("Bytes value for the TCP socket speed limit config")
            .value_name("BYTES COUNT")
            .long(GLOBAL_ARG_TCP_LIMIT_BYTES)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UDP_LIMIT_SHIFT)
            .help("Shift value for the UDP socket speed limit config")
            .value_name("SHIFT VALUE")
            .long(GLOBAL_ARG_UDP_LIMIT_SHIFT)
            .global(true)
            .num_args(1)
            .value_parser(["2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"])
            .default_value("10"),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UDP_LIMIT_BYTES)
            .help("Bytes value for the UDP socket speed limit config")
            .value_name("BYTES COUNT")
            .long(GLOBAL_ARG_UDP_LIMIT_BYTES)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UDP_LIMIT_PACKETS)
            .help("Packet value for the UDP socket speed limit config")
            .value_name("PACKETS COUNT")
            .long(GLOBAL_ARG_UDP_LIMIT_PACKETS)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
}

pub fn parse_global_args(args: &ArgMatches) -> anyhow::Result<ProcArgs> {
    let mut proc_args = ProcArgs::default();

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_CONCURRENCY) {
        proc_args.concurrency = NonZeroUsize::new(*n).unwrap_or(NonZeroUsize::MIN);
    }

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_LATENCY) {
        proc_args.latency = Some(Duration::from_millis(*n as u64));
    }

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_REQUESTS) {
        proc_args.requests = Some(*n);
    }

    if let Some(values) = args.get_many::<String>(GLOBAL_ARG_RESOLVE) {
        for v in values {
            proc_args
                .parse_resolve_value(v)
                .context(format!("invalid resolve string {v}"))?;
        }
    }

    proc_args.time_limit = g3_clap::humanize::get_duration(args, GLOBAL_ARG_TIME_LIMIT)?;
    proc_args.rate_limit = g3_clap::limit::get_rate_limit(args, GLOBAL_ARG_RATE_LIMIT)?;

    if args.get_flag(GLOBAL_ARG_UNAIDED) {
        proc_args.use_unaided_worker = true;
    }
    if args.get_flag(GLOBAL_ARG_UNCONSTRAINED) {
        proc_args.task_unconstrained = true;
    }
    if let Some(n) = args.get_one::<NonZeroUsize>(GLOBAL_ARG_THREADS) {
        proc_args.main_runtime.set_thread_number((*n).get());
        proc_args.worker_runtime.set_thread_number_total(*n);
    }
    if let Some(stack_size) = g3_clap::humanize::get_usize(args, GLOBAL_ARG_THREAD_STACK_SIZE)?
        && stack_size > 0
    {
        proc_args.main_runtime.set_thread_stack_size(stack_size);
        proc_args.worker_runtime.set_thread_stack_size(stack_size);
    }
    #[cfg(feature = "openssl-async-job")]
    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_OPENSSL_ASYNC_JOB_INIT_SIZE) {
        if *n > 0 && !g3_openssl::async_job::async_is_capable() {
            return Err(anyhow!("openssl async job is not supported"));
        }
        proc_args.worker_runtime.set_openssl_async_job_init_size(*n);
    }
    #[cfg(feature = "openssl-async-job")]
    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_OPENSSL_ASYNC_JOB_MAX_SIZE) {
        if *n > 0 && !g3_openssl::async_job::async_is_capable() {
            return Err(anyhow!("openssl async job is not supported"));
        }
        proc_args.worker_runtime.set_openssl_async_job_max_size(*n);
    }

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_LOG_ERROR) {
        proc_args.log_error_count = *n;
    }
    if args.get_flag(GLOBAL_ARG_IGNORE_FATAL_ERROR) {
        proc_args.ignore_fatal_error = true;
    }

    if args.get_flag(GLOBAL_ARG_EMIT_METRICS) {
        let mut config =
            StatsdClientConfig::with_prefix(NodeName::from_str(crate::build::PKG_NAME).unwrap());

        if let Some(addr) = args.get_one::<SocketAddr>(GLOBAL_ARG_STATSD_TARGET_UDP) {
            config.set_backend(StatsdBackend::Udp(*addr, None));
        }
        #[cfg(unix)]
        if let Some(path) = args.get_one::<PathBuf>(GLOBAL_ARG_STATSD_TARGET_UNIX) {
            config.set_backend(StatsdBackend::Unix(path.clone()));
        }

        proc_args.statsd_client_config = Some(config);
    }

    if args.get_flag(GLOBAL_ARG_NO_PROGRESS_BAR) || !stderr().is_terminal() {
        proc_args.no_progress_bar = true;
    }
    proc_args.no_summary = args.get_flag(GLOBAL_ARG_NO_SUMMARY);

    if let Some(s) = args.get_one::<String>(GLOBAL_ARG_PEER_PICK_POLICY) {
        proc_args.peer_pick_policy = SelectivePickPolicy::from_str(s).unwrap();
    }

    if let Some(bytes) = args.get_one::<usize>(GLOBAL_ARG_TCP_LIMIT_BYTES) {
        let shift = args.get_one::<String>(GLOBAL_ARG_TCP_LIMIT_SHIFT).unwrap();
        let shift = u8::from_str(shift).unwrap();
        proc_args.tcp_sock_speed_limit.shift_millis = shift;
        proc_args.tcp_sock_speed_limit.max_north = *bytes;
        proc_args.tcp_sock_speed_limit.max_south = *bytes;
    }

    let mut set_udp_limit = false;
    if let Some(bytes) = args.get_one::<usize>(GLOBAL_ARG_UDP_LIMIT_BYTES) {
        proc_args.udp_sock_speed_limit.max_north_bytes = *bytes;
        proc_args.udp_sock_speed_limit.max_south_bytes = *bytes;
        set_udp_limit = true;
    }
    if let Some(packets) = args.get_one::<usize>(GLOBAL_ARG_UDP_LIMIT_PACKETS) {
        proc_args.udp_sock_speed_limit.max_north_packets = *packets;
        proc_args.udp_sock_speed_limit.max_south_packets = *packets;
        set_udp_limit = true;
    }
    if set_udp_limit {
        let shift = args.get_one::<String>(GLOBAL_ARG_UDP_LIMIT_SHIFT).unwrap();
        let shift = u8::from_str(shift).unwrap();
        proc_args.tcp_sock_speed_limit.shift_millis = shift;
    }

    if proc_args.time_limit.is_none() && proc_args.requests.is_none() {
        proc_args.requests = Some(1);
    }

    proc_args
        .worker_runtime
        .check()
        .context("invalid worker runtime config")?;
    Ok(proc_args)
}
