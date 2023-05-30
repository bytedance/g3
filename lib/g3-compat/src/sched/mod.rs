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

cfg_if::cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "linux"))] {
        mod linux;
        pub use linux::CpuAffinity;
    } else if #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))] {
        mod freebsd;
        pub use freebsd::CpuAffinity;
    } else if #[cfg(target_os = "netbsd")] {
        mod netbsd;
        pub use netbsd::CpuAffinity;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use macos::CpuAffinity;
    } else {
        mod other;
        pub use other::CpuAffinity;
    }
}
