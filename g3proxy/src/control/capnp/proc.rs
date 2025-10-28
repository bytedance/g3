/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::metrics::NodeName;

use g3proxy_proto::escaper_capnp::escaper_control;
use g3proxy_proto::proc_capnp::proc_control;
use g3proxy_proto::resolver_capnp::resolver_control;
use g3proxy_proto::server_capnp::server_control;
use g3proxy_proto::types_capnp::fetch_result;
use g3proxy_proto::user_group_capnp::user_group_control;

use super::set_operation_result;

pub(super) struct ProcControlImpl;

impl proc_control::Server for ProcControlImpl {
    async fn version(
        &self,
        _params: proc_control::VersionParams,
        mut results: proc_control::VersionResults,
    ) -> capnp::Result<()> {
        results.get().set_version(crate::build::VERSION);
        Ok(())
    }

    async fn offline(
        &self,
        _params: proc_control::OfflineParams,
        mut results: proc_control::OfflineResults,
    ) -> capnp::Result<()> {
        g3_daemon::control::quit::start_graceful_shutdown().await;
        set_operation_result(results.get().init_result(), Ok(()));
        Ok(())
    }

    async fn cancel_shutdown(
        &self,
        _params: proc_control::CancelShutdownParams,
        mut results: proc_control::CancelShutdownResults,
    ) -> capnp::Result<()> {
        let r = g3_daemon::control::quit::cancel_graceful_shutdown().await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn release_controller(
        &self,
        _params: proc_control::ReleaseControllerParams,
        mut results: proc_control::ReleaseControllerResults,
    ) -> capnp::Result<()> {
        let r = g3_daemon::control::quit::release_controller().await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_user_group(
        &self,
        _params: proc_control::ListUserGroupParams,
        mut results: proc_control::ListUserGroupResults,
    ) -> capnp::Result<()> {
        let set = crate::auth::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn list_resolver(
        &self,
        _params: proc_control::ListResolverParams,
        mut results: proc_control::ListResolverResults,
    ) -> capnp::Result<()> {
        let set = crate::resolve::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn list_auditor(
        &self,
        _params: proc_control::ListAuditorParams,
        mut results: proc_control::ListAuditorResults,
    ) -> capnp::Result<()> {
        let set = crate::audit::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn list_escaper(
        &self,
        _params: proc_control::ListEscaperParams,
        mut results: proc_control::ListEscaperResults,
    ) -> capnp::Result<()> {
        let set = crate::escape::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn list_server(
        &self,
        _params: proc_control::ListServerParams,
        mut results: proc_control::ListServerResults,
    ) -> capnp::Result<()> {
        let set = crate::serve::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn reload_user_group(
        &self,
        params: proc_control::ReloadUserGroupParams,
        mut results: proc_control::ReloadUserGroupResults,
    ) -> capnp::Result<()> {
        let user_group = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_user_group(user_group, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn reload_resolver(
        &self,
        params: proc_control::ReloadResolverParams,
        mut results: proc_control::ReloadResolverResults,
    ) -> capnp::Result<()> {
        let resolver = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_resolver(resolver, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn reload_auditor(
        &self,
        params: proc_control::ReloadAuditorParams,
        mut results: proc_control::ReloadAuditorResults,
    ) -> capnp::Result<()> {
        let auditor = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_auditor(auditor, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn reload_escaper(
        &self,
        params: proc_control::ReloadEscaperParams,
        mut results: proc_control::ReloadEscaperResults,
    ) -> capnp::Result<()> {
        let escaper = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_escaper(escaper, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn reload_server(
        &self,
        params: proc_control::ReloadServerParams,
        mut results: proc_control::ReloadServerResults,
    ) -> capnp::Result<()> {
        let server = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_server(server, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn get_user_group(
        &self,
        params: proc_control::GetUserGroupParams,
        mut results: proc_control::GetUserGroupResults,
    ) -> capnp::Result<()> {
        let user_group = params.get()?.get_name()?.to_str()?;
        let ug = super::user_group::UserGroupControlImpl::new_client(user_group);
        set_fetch_result::<user_group_control::Owned>(results.get().init_user_group(), Ok(ug))
    }

    async fn get_resolver(
        &self,
        params: proc_control::GetResolverParams,
        mut results: proc_control::GetResolverResults,
    ) -> capnp::Result<()> {
        let resolver = params.get()?.get_name()?.to_str()?;
        set_fetch_result::<resolver_control::Owned>(
            results.get().init_resolver(),
            super::resolver::ResolverControlImpl::new_client(resolver),
        )
    }

    async fn get_escaper(
        &self,
        params: proc_control::GetEscaperParams,
        mut results: proc_control::GetEscaperResults,
    ) -> capnp::Result<()> {
        let escaper = params.get()?.get_name()?.to_str()?;
        set_fetch_result::<escaper_control::Owned>(
            results.get().init_escaper(),
            super::escaper::EscaperControlImpl::new_client(escaper),
        )
    }

    async fn get_server(
        &self,
        params: proc_control::GetServerParams,
        mut results: proc_control::GetServerResults,
    ) -> capnp::Result<()> {
        let server = params.get()?.get_name()?.to_str()?;
        set_fetch_result::<server_control::Owned>(
            results.get().init_server(),
            super::server::ServerControlImpl::new_client(server),
        )
    }

    async fn force_quit_offline_servers(
        &self,
        _params: proc_control::ForceQuitOfflineServersParams,
        mut results: proc_control::ForceQuitOfflineServersResults,
    ) -> capnp::Result<()> {
        crate::serve::force_quit_offline_servers();
        results.get().init_result().set_ok("success");
        Ok(())
    }

    async fn force_quit_offline_server(
        &self,
        params: proc_control::ForceQuitOfflineServerParams,
        mut results: proc_control::ForceQuitOfflineServerResults,
    ) -> capnp::Result<()> {
        let server = params.get()?.get_name()?.to_str()?;
        let server = unsafe { NodeName::new_unchecked(server) };
        crate::serve::force_quit_offline_server(&server);
        results.get().init_result().set_ok("success");
        Ok(())
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
            ev.set_reason(format!("{e:?}").as_str());
            Ok(())
        }
    }
}
