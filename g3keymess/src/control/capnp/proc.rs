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

use std::str::FromStr;

use anyhow::anyhow;
use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::metrics::{MetricsTagName, MetricsTagValue};

use g3keymess_proto::proc_capnp::proc_control;
use g3keymess_proto::server_capnp::server_control;
use g3keymess_proto::types_capnp::fetch_result;

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
            let r = crate::control::bridge::offline().await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn list_server(
        &mut self,
        _params: proc_control::ListServerParams,
        mut results: proc_control::ListServerResults,
    ) -> Promise<(), capnp::Error> {
        let set = crate::serve::get_names();
        let mut builder = results.get().init_result(set.len() as u32);
        for (i, name) in set.iter().enumerate() {
            builder.set(i as u32, name.as_str());
        }
        Promise::ok(())
    }

    fn get_server(
        &mut self,
        params: proc_control::GetServerParams,
        mut results: proc_control::GetServerResults,
    ) -> Promise<(), capnp::Error> {
        let server = pry!(pry!(pry!(params.get()).get_name()).to_str());
        pry!(set_fetch_result::<server_control::Owned>(
            results.get().init_server(),
            super::server::ServerControlImpl::new_client(server),
        ));
        Promise::ok(())
    }

    fn publish_key(
        &mut self,
        params: proc_control::PublishKeyParams,
        mut results: proc_control::PublishKeyResults,
    ) -> Promise<(), capnp::Error> {
        let pem = pry!(pry!(pry!(params.get()).get_pem()).to_string());
        Promise::from_future(async move {
            let r = crate::control::bridge::add_key(&pem).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn list_keys(
        &mut self,
        _params: proc_control::ListKeysParams,
        mut results: proc_control::ListKeysResults,
    ) -> Promise<(), capnp::Error> {
        Promise::from_future(async move {
            let r = crate::control::bridge::list_keys()
                .await
                .unwrap_or_default();
            let mut builder = results.get().init_result(r.len() as u32);
            for (i, ski) in r.iter().enumerate() {
                builder.set(i as u32, ski.as_slice());
            }
            Ok(())
        })
    }

    fn check_key(
        &mut self,
        params: proc_control::CheckKeyParams,
        mut results: proc_control::CheckKeyResults,
    ) -> Promise<(), capnp::Error> {
        let ski = pry!(pry!(params.get()).get_ski()).to_vec();
        Promise::from_future(async move {
            let r = crate::control::bridge::check_key(ski).await;
            set_operation_result(results.get().init_result(), r);
            Ok(())
        })
    }

    fn add_metrics_tag(
        &mut self,
        params: proc_control::AddMetricsTagParams,
        mut results: proc_control::AddMetricsTagResults,
    ) -> Promise<(), capnp::Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_str());
        let value = pry!(pry!(pry!(params.get()).get_value()).to_str());

        let r = do_add_metrics_tag(name, value);
        set_operation_result(results.get().init_result(), r);
        Promise::ok(())
    }
}

fn do_add_metrics_tag(name: &str, value: &str) -> anyhow::Result<()> {
    let name =
        MetricsTagName::from_str(name).map_err(|e| anyhow!("invalid metrics tag name: {e}"))?;
    let value =
        MetricsTagValue::from_str(value).map_err(|e| anyhow!("invalid metrics tag value: {e}"))?;

    // add for server metrics
    crate::serve::foreach_server(|_, s| {
        s.add_dynamic_metrics_tag(name.clone(), value.clone());
    });

    Ok(())
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
