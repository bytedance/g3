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

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3proxy_proto::escaper_capnp::escaper_control;
use g3proxy_proto::proc_capnp::proc_control;
use g3proxy_proto::resolver_capnp::resolver_control;
use g3proxy_proto::server_capnp::server_control;
use g3proxy_proto::types_capnp::fetch_result;
use g3proxy_proto::user_group_capnp::user_group_control;

use super::set_operation_result;

pub(super) struct ProcControlImpl;

impl proc_control::Server for ProcControlImpl {
    fn version(
        &mut self,
        _params: proc_control::VersionParams,
        mut results: proc_control::VersionResults,
    ) -> Promise<(), capnp::Error> {
        results.get().set_version(crate::build::VERSION);
        Promise::ok(())
    }

    fn offline(
        &mut self,
        _params: proc_control::OfflineParams,
        mut results: proc_control::OfflineResults,
    ) -> Promise<(), capnp::Error> {
        Promise::from_future(async move {
            crate::control::DaemonController::abort().await;
            results.get().init_result().set_ok("success");
            Ok(())
        })
    }

    fn list_user_group(
        &mut self,
        _params: proc_control::ListUserGroupParams,
        mut results: proc_control::ListUserGroupResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::auth::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn list_resolver(
        &mut self,
        _params: proc_control::ListResolverParams,
        mut results: proc_control::ListResolverResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::resolve::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn list_auditor(
        &mut self,
        _params: proc_control::ListAuditorParams,
        mut results: proc_control::ListAuditorResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::audit::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn list_escaper(
        &mut self,
        _params: proc_control::ListEscaperParams,
        mut results: proc_control::ListEscaperResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::escape::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name);
        }
        Promise::ok(())
    }

    fn list_server(
        &mut self,
        _params: proc_control::ListServerParams,
        mut results: proc_control::ListServerResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::serve::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name);
        }
        Promise::ok(())
    }

    fn reload_user_group(
        &mut self,
        params: proc_control::ReloadUserGroupParams,
        mut results: proc_control::ReloadUserGroupResults,
    ) -> Promise<(), capnp::Error> {
        let user_group = pry!(pry!(params.get()).get_name()).to_string();
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_user_group(user_group, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn reload_resolver(
        &mut self,
        params: proc_control::ReloadResolverParams,
        mut results: proc_control::ReloadResolverResults,
    ) -> Promise<(), capnp::Error> {
        let resolver = pry!(pry!(params.get()).get_name()).to_string();
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_resolver(resolver, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn reload_auditor(
        &mut self,
        params: proc_control::ReloadAuditorParams,
        mut results: proc_control::ReloadAuditorResults,
    ) -> Promise<(), capnp::Error> {
        let auditor = pry!(pry!(params.get()).get_name()).to_string();
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_auditor(auditor, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn reload_escaper(
        &mut self,
        params: proc_control::ReloadEscaperParams,
        mut results: proc_control::ReloadEscaperResults,
    ) -> Promise<(), capnp::Error> {
        let escaper = pry!(pry!(params.get()).get_name()).to_string();
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_escaper(escaper, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn reload_server(
        &mut self,
        params: proc_control::ReloadServerParams,
        mut results: proc_control::ReloadServerResults,
    ) -> Promise<(), capnp::Error> {
        let server = pry!(pry!(params.get()).get_name()).to_string();
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_server(server, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn get_user_group(
        &mut self,
        params: proc_control::GetUserGroupParams,
        mut results: proc_control::GetUserGroupResults,
    ) -> Promise<(), capnp::Error> {
        let user_group = pry!(pry!(params.get()).get_name());
        let ug = super::user_group::UserGroupControlImpl::new_client(user_group);
        pry!(set_fetch_result::<user_group_control::Owned>(
            results.get().init_user_group(),
            Ok(ug),
        ));
        Promise::ok(())
    }

    fn get_resolver(
        &mut self,
        params: proc_control::GetResolverParams,
        mut results: proc_control::GetResolverResults,
    ) -> Promise<(), capnp::Error> {
        let resolver = pry!(pry!(params.get()).get_name());
        pry!(set_fetch_result::<resolver_control::Owned>(
            results.get().init_resolver(),
            super::resolver::ResolverControlImpl::new_client(resolver),
        ));
        Promise::ok(())
    }

    fn get_escaper(
        &mut self,
        params: proc_control::GetEscaperParams,
        mut results: proc_control::GetEscaperResults,
    ) -> Promise<(), capnp::Error> {
        let escaper = pry!(pry!(params.get()).get_name());
        pry!(set_fetch_result::<escaper_control::Owned>(
            results.get().init_escaper(),
            super::escaper::EscaperControlImpl::new_client(escaper),
        ));
        Promise::ok(())
    }

    fn get_server(
        &mut self,
        params: proc_control::GetServerParams,
        mut results: proc_control::GetServerResults,
    ) -> Promise<(), capnp::Error> {
        let server = pry!(pry!(params.get()).get_name());
        pry!(set_fetch_result::<server_control::Owned>(
            results.get().init_server(),
            super::server::ServerControlImpl::new_client(server),
        ));
        Promise::ok(())
    }

    fn force_quit_offline_servers(
        &mut self,
        _params: proc_control::ForceQuitOfflineServersParams,
        mut results: proc_control::ForceQuitOfflineServersResults,
    ) -> Promise<(), capnp::Error> {
        crate::serve::force_quit_offline_servers();
        results.get().init_result().set_ok("success");
        Promise::ok(())
    }

    fn force_quit_offline_server(
        &mut self,
        params: proc_control::ForceQuitOfflineServerParams,
        mut results: proc_control::ForceQuitOfflineServerResults,
    ) -> Promise<(), capnp::Error> {
        let server = pry!(pry!(params.get()).get_name()).to_string();
        crate::serve::force_quit_offline_server(&server);
        results.get().init_result().set_ok("success");
        Promise::ok(())
    }
}

fn set_fetch_result<'a, T>(
    mut builder: fetch_result::Builder<'a, T>,
    r: anyhow::Result<<T as capnp::traits::Owned>::Reader<'a>>,
) -> capnp::Result<()>
where
    T: capnp::traits::Owned,
{
    match r {
        Ok(data) => builder.set_data(data),
        Err(e) => {
            let mut ev = builder.init_err();
            ev.set_code(-1);
            ev.set_reason(&format!("{e:?}"));
            Ok(())
        }
    }
}
