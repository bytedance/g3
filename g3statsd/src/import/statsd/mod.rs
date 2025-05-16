/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod import;
pub(super) use import::StatsdImporter;

mod parser;
use parser::StatsdRecordVisitor;
