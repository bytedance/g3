/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};
use hickory_proto::rr::{DNSClass, Name, RecordType};

use super::{BenchTarget, BenchTaskContext, ProcArgs};

mod opts;
use opts::BenchDnsArgs;

mod stats;
use stats::{DnsHistogram, DnsHistogramRecorder, DnsRuntimeStats};

mod task;
use task::DnsTaskContext;

pub const COMMAND: &str = "dns";

struct DnsTarget {
    args: Arc<BenchDnsArgs>,
    stats: Arc<DnsRuntimeStats>,
    histogram: Option<DnsHistogram>,
    histogram_recorder: DnsHistogramRecorder,
}

impl BenchTarget<DnsRuntimeStats, DnsHistogram, DnsTaskContext> for DnsTarget {
    fn new_context(&self) -> anyhow::Result<DnsTaskContext> {
        DnsTaskContext::new(&self.args, &self.stats, self.histogram_recorder.clone())
    }

    fn fetch_runtime_stats(&self) -> Arc<DnsRuntimeStats> {
        self.stats.clone()
    }

    fn take_histogram(&mut self) -> Option<DnsHistogram> {
        self.histogram.take()
    }
}

pub fn command() -> Command {
    opts::add_dns_args(Command::new(COMMAND))
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    let dns_args = opts::parse_dns_args(cmd_args)?;

    let (histogram, histogram_recorder) = DnsHistogram::new();
    let target = DnsTarget {
        args: Arc::new(dns_args),
        stats: Arc::new(DnsRuntimeStats::default()),
        histogram: Some(histogram),
        histogram_recorder,
    };

    super::run(target, proc_args).await
}

struct DnsRequest {
    name: Name,
    class: DNSClass,
    rtype: RecordType,
}

impl DnsRequest {
    fn parse_one(s: &str) -> anyhow::Result<Self> {
        let parts = s.split(',').collect::<Vec<&str>>();
        match parts.len() {
            1 => {
                let mut name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name {}: {e}", parts[0]))?;
                name.set_fqdn(true);
                Ok(DnsRequest {
                    name,
                    class: DNSClass::IN,
                    rtype: RecordType::A,
                })
            }
            2 => {
                let mut name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name: {}: {e}", parts[0]))?;
                name.set_fqdn(true);
                let rtype = RecordType::from_str(parts[1])
                    .map_err(|e| anyhow!("invalid record type {}: {e}", parts[1]))?;
                Ok(DnsRequest {
                    name,
                    class: DNSClass::IN,
                    rtype,
                })
            }
            3 => {
                let mut name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name {}: {e}", parts[0]))?;
                name.set_fqdn(true);
                let class = DNSClass::from_str(parts[1])
                    .map_err(|e| anyhow!("invalid class type {}: {e}", parts[1]))?;
                let rtype = RecordType::from_str(parts[2])
                    .map_err(|e| anyhow!("invalid record type {}: {e}", parts[2]))?;
                Ok(DnsRequest { name, class, rtype })
            }
            _ => Err(anyhow!("unsupported request {s}")),
        }
    }
}

trait DnsRequestPickState {
    fn pick_next(&self, max: usize) -> usize;
}
