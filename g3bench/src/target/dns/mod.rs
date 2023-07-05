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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};
use trust_dns_proto::rr::{DNSClass, Name, RecordType};

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
}

impl BenchTarget<DnsRuntimeStats, DnsHistogram, DnsTaskContext> for DnsTarget {
    fn new_context(&self) -> anyhow::Result<DnsTaskContext> {
        let histogram_recorder = self.histogram.as_ref().map(|h| h.recorder());
        DnsTaskContext::new(&self.args, &self.stats, histogram_recorder)
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

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    let dns_args = opts::parse_dns_args(cmd_args)?;

    let target = DnsTarget {
        args: Arc::new(dns_args),
        stats: Arc::new(DnsRuntimeStats::default()),
        histogram: Some(DnsHistogram::new()),
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
                let name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name {}: {e}", parts[0]))?;
                Ok(DnsRequest {
                    name,
                    class: DNSClass::IN,
                    rtype: RecordType::A,
                })
            }
            2 => {
                let name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name: {}: {e}", parts[0]))?;
                let rtype = RecordType::from_str(parts[1])
                    .map_err(|e| anyhow!("invalid record type {}: {e}", parts[1]))?;
                Ok(DnsRequest {
                    name,
                    class: DNSClass::IN,
                    rtype,
                })
            }
            3 => {
                let name = Name::from_utf8(parts[0])
                    .map_err(|e| anyhow!("invalid domain name {}: {e}", parts[0]))?;
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
