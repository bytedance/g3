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

mod custom;
mod standard;

pub(crate) use custom::{
    dynamic_egress_info, outgoing_ip, remote_connection_info, set_dynamic_egress_info,
    set_outgoing_ip, set_remote_connection_info, set_upstream_addr, set_upstream_id, upstream_addr,
};
pub(crate) use standard::proxy_authorization_basic_pass;
