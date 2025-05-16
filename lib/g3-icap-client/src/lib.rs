/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod parse;
mod reason;
mod serialize;

pub mod reqmod;

pub mod respmod;

mod options;
pub use options::IcapServiceOptions;

mod service;

use service::{IcapClientConnection, IcapClientReader, IcapClientWriter};
pub use service::{IcapMethod, IcapServiceClient, IcapServiceConfig};
