/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod daemon;
pub use daemon::{DaemonArgs, DaemonArgsExt};

mod control;
pub use control::{DEFAULT_CONTROL_DIR, control_dir, validate_and_set_control_dir};

mod config;
pub use config::{config_dir, config_file, config_file_extension, validate_and_set_config_file};
