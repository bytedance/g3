/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use capnp::capability::Promise;

use g3_types::metrics::NodeName;

use g3tiles_proto::backend_capnp::backend_control;

use crate::backend::ArcBackend;

pub(super) struct BackendControlImpl {
    backend: ArcBackend,
}

impl BackendControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<backend_control::Client> {
        let name = unsafe { NodeName::new_unchecked(name) };
        let backend = crate::backend::get_backend(&name)?;
        Ok(capnp_rpc::new_client(BackendControlImpl { backend }))
    }
}

impl backend_control::Server for BackendControlImpl {
    fn alive_connection(
        &mut self,
        _params: backend_control::AliveConnectionParams,
        mut results: backend_control::AliveConnectionResults,
    ) -> Promise<(), capnp::Error> {
        let alive_count = self.backend.alive_connection();
        results.get().set_count(alive_count);
        Promise::ok(())
    }
}
