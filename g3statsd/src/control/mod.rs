/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod bridge;

mod quit;
pub use quit::QuitActor;

mod upgrade;
pub use upgrade::UpgradeActor;

mod local;
pub use local::{DaemonController, UniqueController};

pub mod capnp;
