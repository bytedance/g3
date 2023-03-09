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

use std::net::IpAddr;
use std::str::FromStr;

use log::info;
use slog::{slog_o, Drain};
use slog_scope::GlobalLoggerGuard;

use g3_resolver::{
    driver::trust_dns::TrustDnsDriverConfig, AnyResolveDriverConfig, ResolverBuilder,
    ResolverConfig,
};
use g3_types::log::AsyncLogConfig;

fn setup_log() -> Result<GlobalLoggerGuard, log::SetLoggerError> {
    let async_conf = AsyncLogConfig::default();
    let drain = g3_stdlog::new_async_logger(&async_conf, true);
    let logger = slog::Logger::root(drain.fuse(), slog_o!());

    let scope_guard = slog_scope::set_global_logger(logger);

    slog_stdlog::init_with_level(log::Level::Trace)?;
    Ok(scope_guard)
}

fn main() {
    let _logger_guard = setup_log().unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut config = TrustDnsDriverConfig::default();
        config.add_server(IpAddr::from_str("223.5.5.5").unwrap());
        let config = ResolverConfig {
            name: String::new(),
            driver: AnyResolveDriverConfig::TrustDns(config),
            runtime: Default::default(),
        };
        let resolver = ResolverBuilder::new(config).build().unwrap();
        let handle = resolver.get_handle();
        let mut job = handle.get_v4("www.xjtu.edu.cn".to_string()).unwrap();
        let data = job.recv().await;
        info!("data: {:?}", data);
        let mut job = handle.get_v4("www.xjtu.edu.cn".to_string()).unwrap();
        let data = job.recv().await;
        info!("data: {:?}", data);
    });
}
