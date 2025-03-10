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

    fn list_input(
        &mut self,
        _params: proc_control::ListInputParams,
        mut results: proc_control::ListInputResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::input::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn reload_input(
        &mut self,
        params: proc_control::ReloadInputParams,
        mut results: proc_control::ReloadInputResults,
    ) -> Promise<(), capnp::Error> {
        let server = pry!(pry!(pry!(params.get()).get_name()).to_string());
        Promise::from_future(async move {
            let r = crate::control::bridge::reload_input(server, None).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
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
