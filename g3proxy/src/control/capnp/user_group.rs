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

use std::sync::Arc;

use capnp::capability::Promise;

use g3proxy_proto::user_group_capnp::user_group_control;

use crate::auth::UserGroup;

pub(super) struct UserGroupControlImpl {
    user_group: Arc<UserGroup>,
}

impl UserGroupControlImpl {
    pub(super) fn new_client(name: &str) -> user_group_control::Client {
        let user_group = crate::auth::get_or_insert_default(name);
        capnp_rpc::new_client(UserGroupControlImpl { user_group })
    }
}

impl user_group_control::Server for UserGroupControlImpl {
    fn list_static_user(
        &mut self,
        _params: user_group_control::ListStaticUserParams,
        mut results: user_group_control::ListStaticUserResults,
    ) -> Promise<(), capnp::Error> {
        let v = self.user_group.all_static_users();
        let mut builder = results.get().init_result(v.len() as u32);
        for (i, name) in v.iter().enumerate() {
            builder.set(i as u32, name);
        }
        Promise::ok(())
    }

    fn list_dynamic_user(
        &mut self,
        _params: user_group_control::ListDynamicUserParams,
        mut results: user_group_control::ListDynamicUserResults,
    ) -> Promise<(), capnp::Error> {
        let v = self.user_group.all_dynamic_users();
        let mut builder = results.get().init_result(v.len() as u32);
        for (i, name) in v.iter().enumerate() {
            builder.set(i as u32, name);
        }
        Promise::ok(())
    }
}
