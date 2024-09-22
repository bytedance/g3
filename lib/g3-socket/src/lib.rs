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

#[cfg(any(target_os = "linux", target_os = "android"))]
mod sockopt;

mod raw;
pub use raw::RawSocket;

pub mod tcp;
pub mod udp;
pub mod util;

#[cfg(unix)]
mod interface;
#[cfg(unix)]
pub use interface::InterfaceName;

mod bind;
pub use bind::BindAddr;
