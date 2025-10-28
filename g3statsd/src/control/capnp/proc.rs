/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use g3statsd_proto::proc_capnp::proc_control;
use g3statsd_proto::types_capnp::fetch_result;

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

    async fn list_importer(
        &self,
        _params: proc_control::ListImporterParams,
        mut results: proc_control::ListImporterResults,
    ) -> capnp::Result<()> {
        let set = crate::import::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn reload_importer(
        &self,
        params: proc_control::ReloadImporterParams,
        mut results: proc_control::ReloadImporterResults,
    ) -> capnp::Result<()> {
        let name = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_importer(name, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_collector(
        &self,
        _params: proc_control::ListCollectorParams,
        mut results: proc_control::ListCollectorResults,
    ) -> capnp::Result<()> {
        let set = crate::collect::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn reload_collector(
        &self,
        params: proc_control::ReloadCollectorParams,
        mut results: proc_control::ReloadCollectorResults,
    ) -> capnp::Result<()> {
        let name = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_collector(name, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_exporter(
        &self,
        _params: proc_control::ListExporterParams,
        mut results: proc_control::ListExporterResults,
    ) -> capnp::Result<()> {
        let set = crate::export::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Ok(())
    }

    async fn reload_exporter(
        &self,
        params: proc_control::ReloadExporterParams,
        mut results: proc_control::ReloadExporterResults,
    ) -> capnp::Result<()> {
        let name = params.get()?.get_name()?.to_string()?;
        let r = crate::control::bridge::reload_exporter(name, None).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }
}

#[allow(unused)]
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
