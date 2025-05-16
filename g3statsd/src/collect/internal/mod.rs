/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod emit;
use emit::InternalEmitter;

mod collect;
pub(super) use collect::InternalCollector;
