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

use std::io::{stderr, IsTerminal};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use ahash::AHashMap;
use anyhow::{anyhow, Context};
use cadence::StatsdClient;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};

use g3_runtime::blended::BlendedRuntimeConfig;
use g3_runtime::unaided::UnaidedRuntimeConfig;
use g3_statsd::client::{StatsdBackend, StatsdClientConfig};
use g3_types::collection::{SelectivePickPolicy, SelectiveVec, SelectiveVecBuilder, WeightedValue};
use g3_types::metrics::MetricsName;
use g3_types::net::{TcpSockSpeedLimitConfig, UpstreamAddr};

use super::progress::BenchProgress;

const GLOBAL_ARG_UNAIDED: &str = "unaided";
const GLOBAL_ARG_UNCONSTRAINED: &str = "unconstrained";
const GLOBAL_ARG_THREADS: &str = "threads";
const GLOBAL_ARG_THREAD_STACK_SIZE: &str = "thread-stack-size";
const GLOBAL_ARG_CONCURRENCY: &str = "concurrency";
const GLOBAL_ARG_TIME_LIMIT: &str = "time-limit";
const GLOBAL_ARG_REQUESTS: &str = "requests";
const GLOBAL_ARG_RESOLVE: &str = "resolve";
const GLOBAL_ARG_LOG_ERROR: &str = "log-error";
const GLOBAL_ARG_EMIT_METRICS: &str = "emit-metrics";
const GLOBAL_ARG_STATSD_TARGET_UDP: &str = "statsd-target-udp";
const GLOBAL_ARG_STATSD_TARGET_UNIX: &str = "statsd-target-unix";
const GLOBAL_ARG_NO_PROGRESS_BAR: &str = "no-progress-bar";

const GLOBAL_ARG_PEER_PICK_POLICY: &str = "peer-pick-policy";
const GLOBAL_ARG_TCP_LIMIT_SHIFT: &str = "tcp-limit-shift";
const GLOBAL_ARG_TCP_LIMIT_BYTES: &str = "tcp-limit-bytes";

const DEFAULT_STAT_PREFIX: &str = "g3bench";

pub struct ProcArgs {
    pub(super) concurrency: usize,
    pub(super) requests: Option<usize>,
    pub(super) time_limit: Option<Duration>,
    pub(super) log_error_count: usize,
    pub(super) task_unconstrained: bool,
    resolver: AHashMap<UpstreamAddr, IpAddr>,
    use_unaided_worker: bool,
    thread_number: Option<usize>,
    thread_stack_size: Option<usize>,

    statsd_client_config: Option<StatsdClientConfig>,
    no_progress_bar: bool,

    peer_pick_policy: SelectivePickPolicy,
    pub(super) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            concurrency: 1,
            requests: None,
            time_limit: None,
            log_error_count: 0,
            task_unconstrained: false,
            resolver: AHashMap::new(),
            use_unaided_worker: false,
            thread_number: None,
            thread_stack_size: None,
            statsd_client_config: None,
            no_progress_bar: false,
            peer_pick_policy: SelectivePickPolicy::RoundRobin,
            tcp_sock_speed_limit: TcpSockSpeedLimitConfig::default(),
        }
    }
}

impl ProcArgs {
    pub fn summary(&self) {
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
                Ok(builder) => {
                    let start_instant = Instant::now();
                    let client = builder
                        .with_error_handler(move |e| {
                            static mut LAST_REPORT_TIME_SLICE: u64 = 0;
                            let time_slice = start_instant.elapsed().as_secs().rotate_right(6); // every 64s
                            unsafe {
                                if LAST_REPORT_TIME_SLICE != time_slice {
                                    eprintln!("sending metrics error: {e:?}");
                                    LAST_REPORT_TIME_SLICE = time_slice;
                                }
                            }
                        })
                        .with_tag("pid", std::process::id())
                        .build();
                    Some((client, config.emit_duration))
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
            .map_err(|e| anyhow!("failed to build vec: {e}"))
    }

    pub(super) fn select_peer<'a, T>(&'a self, peers: &'a SelectiveVec<WeightedValue<T>>) -> &'a T {
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
            let mut runtime = BlendedRuntimeConfig::new();
            if let Some(thread_number) = self.thread_number {
                runtime.set_thread_number(thread_number);
            }
            if let Some(thread_stack_size) = self.thread_stack_size {
                runtime.set_thread_stack_size(thread_stack_size);
            }
            runtime
        }
    }

    pub fn worker_runtime(&self) -> Option<UnaidedRuntimeConfig> {
        if self.use_unaided_worker {
            let mut runtime = UnaidedRuntimeConfig::new();
            if let Some(thread_number) = self.thread_number {
                runtime.set_thread_number(thread_number);
            }
            if let Some(thread_stack_size) = self.thread_stack_size {
                runtime.set_thread_stack_size(thread_stack_size);
            }
            Some(runtime)
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
        Arg::new(GLOBAL_ARG_TIME_LIMIT)
            .help("Maximum time to spend for benchmarking")
            .value_name("TOTAL TIME")
            .global(true)
            .short('t')
            .long(GLOBAL_ARG_TIME_LIMIT)
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
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_THREAD_STACK_SIZE)
            .long(GLOBAL_ARG_THREAD_STACK_SIZE)
            .value_name("STACK SIZE")
            .global(true)
            .num_args(1),
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
            .help("Shift value for the TCP per connection rate limit config")
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
            .help("Bytes value for the TCP per connect rate limit config")
            .value_name("BYTES COUNT")
            .long(GLOBAL_ARG_TCP_LIMIT_BYTES)
            .global(true)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
}

pub fn parse_global_args(args: &ArgMatches) -> anyhow::Result<ProcArgs> {
    let mut proc_args = ProcArgs::default();

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_CONCURRENCY) {
        proc_args.concurrency = *n;
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

    if args.get_flag(GLOBAL_ARG_UNAIDED) {
        proc_args.use_unaided_worker = true;
    }
    if args.get_flag(GLOBAL_ARG_UNCONSTRAINED) {
        proc_args.task_unconstrained = true;
    }
    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_THREADS) {
        proc_args.thread_number = Some(*n);
    }
    if let Some(stack_size) = g3_clap::humanize::get_usize(args, GLOBAL_ARG_THREAD_STACK_SIZE)? {
        if stack_size > 0 {
            proc_args.thread_stack_size = Some(stack_size);
        }
    }

    if let Some(n) = args.get_one::<usize>(GLOBAL_ARG_LOG_ERROR) {
        proc_args.log_error_count = *n;
    }

    if args.get_flag(GLOBAL_ARG_EMIT_METRICS) {
        let mut config =
            StatsdClientConfig::with_prefix(MetricsName::from_str(DEFAULT_STAT_PREFIX).unwrap());

        if let Some(addr) = args.get_one::<SocketAddr>(GLOBAL_ARG_STATSD_TARGET_UDP) {
            config.set_backend(StatsdBackend::Udp(*addr, None));
        }
        if let Some(path) = args.get_one::<PathBuf>(GLOBAL_ARG_STATSD_TARGET_UNIX) {
            config.set_backend(StatsdBackend::Unix(path.clone()));
        }

        proc_args.statsd_client_config = Some(config);
    }

    if args.get_flag(GLOBAL_ARG_NO_PROGRESS_BAR) || !stderr().is_terminal() {
        proc_args.no_progress_bar = true;
    }

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

    if proc_args.time_limit.is_none() && proc_args.requests.is_none() {
        proc_args.requests = Some(1);
    }

    Ok(proc_args)
}
