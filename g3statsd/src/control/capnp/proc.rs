/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3statsd_proto::proc_capnp::proc_control;
use g3statsd_proto::types_capnp::fetch_result;

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
            g3_daemon::control::quit::start_graceful_shutdown().await;
            set_operation_result(results.get().init_result(), Ok(()));
            Ok(())
        })
    }

    fn cancel_shutdown(
        &mut self,
        _params: proc_control::CancelShutdownParams,
        mut results: proc_control::CancelShutdownResults,
    ) -> Promise<(), capnp::Error> {
        Promise::from_future(async move {
            let r = g3_daemon::control::quit::cancel_graceful_shutdown().await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn release_controller(
        &mut self,
        _params: proc_control::ReleaseControllerParams,
        mut results: proc_control::ReleaseControllerResults,
    ) -> Promise<(), capnp::Error> {
        Promise::from_future(async move {
            let r = g3_daemon::control::quit::release_controller().await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn list_importer(
        &mut self,
        _params: proc_control::ListImporterParams,
        mut results: proc_control::ListImporterResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::import::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn reload_importer(
        &mut self,
        params: proc_control::ReloadImporterParams,
        mut results: proc_control::ReloadImporterResults,
    ) -> Promise<(), capnp::Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_string());
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_importer(name, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn list_collector(
        &mut self,
        _params: proc_control::ListCollectorParams,
        mut results: proc_control::ListCollectorResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::collect::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn reload_collector(
        &mut self,
        params: proc_control::ReloadCollectorParams,
        mut results: proc_control::ReloadCollectorResults,
    ) -> Promise<(), capnp::Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_string());
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_collector(name, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn list_exporter(
        &mut self,
        _params: proc_control::ListExporterParams,
        mut results: proc_control::ListExporterResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::export::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn reload_exporter(
        &mut self,
        params: proc_control::ReloadExporterParams,
        mut results: proc_control::ReloadExporterResults,
    ) -> Promise<(), capnp::Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_string());
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_exporter(name, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
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
