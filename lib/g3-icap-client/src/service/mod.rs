/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod config;
pub use config::IcapServiceConfig;

mod connection;
pub(super) use connection::{IcapClientConnection, IcapClientReader, IcapClientWriter};
use connection::{IcapConnectionEofPoller, IcapConnectionPollRequest, IcapConnector};

mod client;
pub use client::IcapServiceClient;

mod pool;
use pool::{IcapServiceClientCommand, IcapServicePool};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcapMethod {
    Options,
    Reqmod,
    Respmod,
}

impl IcapMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            IcapMethod::Options => "OPTIONS",
            IcapMethod::Reqmod => "REQMOD",
            IcapMethod::Respmod => "RESPMOD",
        }
    }
}
