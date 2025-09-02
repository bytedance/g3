/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::Context;
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command, value_parser};

use super::protocol::{BinaryMessageBuilder, CompactMessageBuilder, ThriftMessageBuilder};

const ARG_METHOD: &str = "method";
const ARG_PAYLOAD: &str = "payload";
const ARG_BINARY: &str = "binary";
const ARG_COMPACT: &str = "compact";
const ARG_CHECK_MESSAGE_LENGTH: &str = "check-message-length";

const ARG_GROUP_PROTOCOL: &str = "protocol";

pub(super) trait AppendThriftArgs {
    fn append_thrift_args(self) -> Self;
}

pub(super) struct ThriftGlobalArgs {
    pub(super) method: String,
    pub(super) payload: Arc<[u8]>,
    pub(super) request_builder: ThriftMessageBuilder,
    pub(super) check_message_length: Option<usize>,
}

impl ThriftGlobalArgs {
    pub(super) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let name = args.get_one::<String>(ARG_METHOD).unwrap();
        let payload = g3_clap::data::get(args, ARG_PAYLOAD, true)?;

        let request_builder = if args.get_flag(ARG_BINARY) {
            let request = BinaryMessageBuilder::new_call(name)
                .context("failed to build thrift binary transport request")?;
            ThriftMessageBuilder::Binary(request)
        } else if args.get_flag(ARG_COMPACT) {
            let request = CompactMessageBuilder::new_call(name)
                .context("failed to build thrift compact transport request")?;
            ThriftMessageBuilder::Compact(request)
        } else {
            unreachable!()
        };

        let check_message_length = args.get_one::<usize>(ARG_CHECK_MESSAGE_LENGTH).copied();

        Ok(ThriftGlobalArgs {
            method: name.to_string(),
            payload: Arc::from(payload),
            request_builder,
            check_message_length,
        })
    }
}

impl AppendThriftArgs for Command {
    fn append_thrift_args(self) -> Self {
        self.arg(
            Arg::new(ARG_METHOD)
                .help("RPC method name")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new(ARG_PAYLOAD)
                .help("Request struct in hex encoding")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new(ARG_BINARY)
                .help("Use binary protocol")
                .long(ARG_BINARY)
                .action(ArgAction::SetTrue)
                .conflicts_with(ARG_COMPACT),
        )
        .arg(
            Arg::new(ARG_COMPACT)
                .help("Use compact protocol")
                .long(ARG_COMPACT)
                .action(ArgAction::SetTrue)
                .conflicts_with(ARG_BINARY),
        )
        .group(
            ArgGroup::new(ARG_GROUP_PROTOCOL)
                .required(true)
                .args([ARG_BINARY, ARG_COMPACT]),
        )
        .arg(
            Arg::new(ARG_CHECK_MESSAGE_LENGTH)
                .help("Check the message length in response")
                .long(ARG_CHECK_MESSAGE_LENGTH)
                .num_args(1)
                .value_parser(value_parser!(usize)),
        )
    }
}
