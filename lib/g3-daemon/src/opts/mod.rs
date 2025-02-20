/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

mod daemon;
pub use daemon::{DaemonArgs, DaemonArgsExt};

mod control;
pub use control::{DEFAULT_CONTROL_DIR, control_dir, validate_and_set_control_dir};

mod config;
pub use config::{config_dir, config_file, config_file_extension, validate_and_set_config_file};
