/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod parser;
use parser::StatsdRecordVisitor;

mod udp;
pub(super) use udp::StatsdUdpImporter;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(super) use unix::StatsdUnixImporter;
