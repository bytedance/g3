/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::metrics::NodeName;

use g3tiles_proto::backend_capnp::backend_control;
use g3tiles_proto::proc_capnp::proc_control;
use g3tiles_proto::server_capnp::server_control;
use g3tiles_proto::types_capnp::fetch_result;

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

    async fn reload_discover(
        &self,
        params: proc_control::ReloadDiscoverParams,
        mut results: proc_control::ReloadDiscoverResults,
    ) -> capnp::Result<()> {
        let discover = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_discover(discover, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_discover(
        &self,
        _params: proc_control::ListDiscoverParams,
        mut results: proc_control::ListDiscoverResults,
    ) -> capnp::Result<()> {
        let set = crate::discover::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn reload_backend(
        &self,
        params: proc_control::ReloadBackendParams,
        mut results: proc_control::ReloadBackendResults,
    ) -> capnp::Result<()> {
        let backend = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_backend(backend, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_backend(
        &self,
        _params: proc_control::ListBackendParams,
        mut results: proc_control::ListBackendResults,
    ) -> capnp::Result<()> {
        let set = crate::backend::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn get_backend(
        &self,
        params: proc_control::GetBackendParams,
        mut results: proc_control::GetBackendResults,
    ) -> capnp::Result<()> {
        let backend = params.get()?.get_name()?.to_str()?;
        set_fetch_result::<backend_control::Owned>(
            results.get().init_backend(),
            super::backend::BackendControlImpl::new_client(backend),
        )
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
