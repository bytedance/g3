/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::rc::Rc;
use std::sync::Arc;

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
    async fn list_static_user(
        self: Rc<Self>,
        _params: user_group_control::ListStaticUserParams,
        mut results: user_group_control::ListStaticUserResults,
    ) -> capnp::Result<()> {
        let v = self.user_group.all_static_users();
        let mut builder = results.get().init_result(v.len() as u32);
        for (i, name) in v.into_iter().enumerate() {
            builder.set(i as u32, name);
        }
        Ok(())
    }

    async fn list_dynamic_user(
        self: Rc<Self>,
        _params: user_group_control::ListDynamicUserParams,
        mut results: user_group_control::ListDynamicUserResults,
    ) -> capnp::Result<()> {
        let v = self.user_group.all_dynamic_users();
        let mut builder = results.get().init_result(v.len() as u32);
        for (i, name) in v.iter().enumerate() {
            builder.set(i as u32, name);
        }
        Ok(())
    }

    async fn publish_dynamic_user(
        self: Rc<Self>,
        params: user_group_control::PublishDynamicUserParams,
        mut results: user_group_control::PublishDynamicUserResults,
    ) -> capnp::Result<()> {
        let user_group = self.user_group.clone();
        let contents = params.get()?.get_contents()?.to_str()?;
        let r = user_group.publish_dynamic_users(contents).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }
}
