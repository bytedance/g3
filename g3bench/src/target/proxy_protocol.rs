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

use anyhow::anyhow;
use clap::{Arg, ArgMatches, Command};

use g3_types::net::{ProxyProtocolEncoder, ProxyProtocolVersion};

const PP_ARG_VERSION: &str = "proxy-protocol-version";
const PP_ARG_ADDRESS: &str = "proxy-protocol";

const PROTOCOL_VERSION_LIST: [&str; 2] = ["1", "2"];
const DEFAULT_PROTOCOL_VERSION: &str = "2";

pub(crate) trait AppendProxyProtocolArgs {
    fn append_proxy_protocol_args(self) -> Self;
}

impl AppendProxyProtocolArgs for Command {
    fn append_proxy_protocol_args(self) -> Self {
        self.arg(
            Arg::new(PP_ARG_VERSION)
                .help("PROXY Protocol version")
                .value_name("VERSION")
                .num_args(1)
                .long(PP_ARG_VERSION)
                .value_parser(PROTOCOL_VERSION_LIST)
                .default_value(DEFAULT_PROTOCOL_VERSION),
        )
        .arg(
            Arg::new(PP_ARG_ADDRESS)
                .help("PROXY protocol address")
                .value_name("CLIENT_ADDR,SERVER_ADDR")
                .num_args(1)
                .long(PP_ARG_ADDRESS),
        )
    }
}

pub(crate) struct ProxyProtocolArgs {
    version: ProxyProtocolVersion,
    data: Option<Vec<u8>>,
}

impl Default for ProxyProtocolArgs {
    fn default() -> Self {
        ProxyProtocolArgs {
            version: ProxyProtocolVersion::V2,
            data: None,
        }
    }
}

impl ProxyProtocolArgs {
    fn parse_version(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(v) = args.get_one::<String>(PP_ARG_VERSION) {
            self.version = ProxyProtocolVersion::from_str(v)?;
        }
        Ok(())
    }

    fn parse_address(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        let Some(v) = args.get_one::<String>(PP_ARG_ADDRESS) else {
            return Ok(());
        };

        let r: Vec<&str> = v.splitn(2, ',').collect();
        if r.len() != 2 {
            return Err(anyhow!("invalid proxy protocol address value"));
        }

        let client_addr =
            SocketAddr::from_str(r[1]).map_err(|e| anyhow!("invalid client address: {e}"))?;
        let server_addr =
            SocketAddr::from_str(r[2]).map_err(|e| anyhow!("invalid server address: {e}"))?;

        let mut encoder = ProxyProtocolEncoder::new(self.version);
        let buf = encoder
            .encode_tcp(client_addr, server_addr)
            .map_err(|e| anyhow!("proxy protocol encode failed: {e}"))?;
        self.data = Some(buf.to_vec());
        Ok(())
    }

    pub(crate) fn parse_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        self.parse_version(args)?;
        self.parse_address(args)
    }

    pub(crate) fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}
