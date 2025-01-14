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

use std::net::IpAddr;

use g3_types::collection::{SelectiveItem, SelectivePickPolicy, SelectiveVec};
use g3_types::metrics::NodeName;

use super::ClientConnectionInfo;

#[derive(Clone)]
pub enum ServerReloadCommand {
    QuitRuntime,
    ReloadVersion(usize),
}

pub trait BaseServer {
    fn name(&self) -> &NodeName;
    fn server_type(&self) -> &'static str;
    fn version(&self) -> usize;
}

pub trait ServerExt: BaseServer {
    fn select_consistent<'a, T>(
        &self,
        nodes: &'a SelectiveVec<T>,
        pick_policy: SelectivePickPolicy,
        cc_info: &ClientConnectionInfo,
    ) -> &'a T
    where
        T: SelectiveItem,
    {
        #[derive(Hash)]
        struct ConsistentKey {
            client_ip: IpAddr,
            server_ip: IpAddr,
        }

        match pick_policy {
            SelectivePickPolicy::Random => nodes.pick_random(),
            SelectivePickPolicy::Serial => nodes.pick_serial(),
            SelectivePickPolicy::RoundRobin => nodes.pick_round_robin(),
            SelectivePickPolicy::Ketama => {
                let key = ConsistentKey {
                    client_ip: cc_info.client_ip(),
                    server_ip: cc_info.server_ip(),
                };
                nodes.pick_ketama(&key)
            }
            SelectivePickPolicy::Rendezvous => {
                let key = ConsistentKey {
                    client_ip: cc_info.client_ip(),
                    server_ip: cc_info.server_ip(),
                };
                nodes.pick_rendezvous(&key)
            }
            SelectivePickPolicy::JumpHash => {
                let key = ConsistentKey {
                    client_ip: cc_info.client_ip(),
                    server_ip: cc_info.server_ip(),
                };
                nodes.pick_jump(&key)
            }
        }
    }
}
