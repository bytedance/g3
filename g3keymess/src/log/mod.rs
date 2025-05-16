/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod shared;

pub(crate) mod request;
pub(crate) mod task;

const LOG_TYPE_TASK: &str = "Task";
const LOG_TYPE_REQUEST: &str = "Request";
