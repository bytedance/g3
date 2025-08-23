/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod shared;

pub(crate) mod audit;
pub(crate) mod escape;
pub(crate) mod inspect;
pub(crate) mod intercept;
pub(crate) mod resolve;
pub(crate) mod task;

const LOG_TYPE_TASK: &str = "Task";
const LOG_TYPE_ESCAPE: &str = "Escape";
const LOG_TYPE_RESOLVE: &str = "Resolve";
const LOG_TYPE_INSPECT: &str = "Inspect";
const LOG_TYPE_INTERCEPT: &str = "Intercept";
