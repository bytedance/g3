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

use std::net::SocketAddr;
use std::str::FromStr;

use tokio::runtime::Builder;

use g3_resolver::{
    driver::c_ares::CAresDriverConfig, AnyResolveDriverConfig, ResolverBuilder, ResolverConfig,
};

#[test]
fn simple_query() {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let mut cares_config = CAresDriverConfig::default();
        cares_config.add_server(SocketAddr::from_str("1.1.1.1:53").unwrap());
        let config = ResolverConfig {
            name: String::new(),
            driver: AnyResolveDriverConfig::CAres(cares_config),
            runtime: Default::default(),
        };
        let resolver = ResolverBuilder::new(config).build().unwrap();
        let handle = resolver.get_handle();
        let mut job = handle.get_v4("www.xjtu.edu.cn".to_string()).unwrap();
        let data = job.recv().await;
        assert!(data.is_ok());
        let mut job = handle.get_v4("www.xjtu.edu.cn".to_string()).unwrap();
        let data = job.recv().await;
        assert!(data.is_ok());
    });
}
