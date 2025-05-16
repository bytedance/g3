/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use async_trait::async_trait;

use crate::server::BaseServer;

#[async_trait]
pub trait AcceptQuicServer: BaseServer {}

pub trait ListenQuicConf {}

pub struct ListenQuicRuntime {}
