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

#[cfg_attr(any(target_os = "linux", target_os = "android"), path = "linux.rs")]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "dragonfly"),
    path = "freebsd.rs"
)]
#[cfg_attr(target_os = "netbsd", path = "netbsd.rs")]
#[cfg_attr(target_os = "macos", path = "macos.rs")]
mod os;
pub use os::CpuAffinity;
