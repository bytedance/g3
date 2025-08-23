/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::metrics::NodeName;

use g3proxy_proto::user_group_capnp::user_group_control;

use super::set_operation_result;
use crate::auth::UserGroup;

pub(super) struct UserGroupControlImpl {
    user_group: Arc<UserGroup>,
}

impl UserGroupControlImpl {
    pub(super) fn new_client(name: &str) -> user_group_control::Client {
        let name = unsafe { NodeName::new_unchecked(name) };
        let user_group = crate::auth::get_or_insert_default(&name);
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
        for (i, name) in v.into_iter().enumerate() {
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

    fn publish_dynamic_user(
        &mut self,
        params: user_group_control::PublishDynamicUserParams,
        mut results: user_group_control::PublishDynamicUserResults,
    ) -> Promise<(), capnp::Error> {
        let user_group = self.user_group.clone();
        let contents = pry!(pry!(pry!(params.get()).get_contents()).to_string());
        Promise::from_future(async move {
            let r = user_group.publish_dynamic_users(&contents).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }
}
