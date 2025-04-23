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

#![allow(clippy::needless_lifetimes)]

pub mod types_capnp {
    #![allow(clippy::extra_unused_type_parameters)]
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/types_capnp.rs"));
}

pub mod proc_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/proc_capnp.rs"));
}
