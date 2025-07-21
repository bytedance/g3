/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod builder;
pub(crate) use builder::BinaryMessageBuilder;

mod parser;
pub(super) use parser::BinaryMessageParser;
