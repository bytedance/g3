/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use super::{AnyImporterConfig, ImporterConfig, ImporterConfigDiffAction};
use super::{CONFIG_KEY_IMPORTER_NAME, CONFIG_KEY_IMPORTER_TYPE};

mod udp;
pub(crate) use udp::StatsdUdpImporterConfig;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::StatsdUnixImporterConfig;
