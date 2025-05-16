/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod collect;
pub(crate) use collect::AggregateCollector;

mod store;
use store::AggregateHandle;
