/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::rc::Rc;
use std::str::FromStr;

use anyhow::anyhow;

use g3_types::metrics::{MetricTagName, MetricTagValue};

use g3keymess_proto::proc_capnp::proc_control;
use g3keymess_proto::server_capnp::server_control;
use g3keymess_proto::types_capnp::fetch_result;

use super::set_operation_result;

pub(super) struct ProcControlImpl;

impl proc_control::Server for ProcControlImpl {
    async fn version(
        self: Rc<Self>,
        _params: proc_control::VersionParams,
        mut results: proc_control::VersionResults,
    ) -> capnp::Result<()> {
        results.get().set_version(crate::build::VERSION);
        Ok(())
    }

    async fn offline(
        self: Rc<Self>,
        _params: proc_control::OfflineParams,
        mut results: proc_control::OfflineResults,
    ) -> capnp::Result<()> {
        g3_daemon::control::quit::start_graceful_shutdown().await;
        set_operation_result(results.get().init_result(), Ok(()));
        Ok(())
    }

    async fn cancel_shutdown(
        self: Rc<Self>,
        _params: proc_control::CancelShutdownParams,
        mut results: proc_control::CancelShutdownResults,
    ) -> capnp::Result<()> {
        let r = g3_daemon::control::quit::cancel_graceful_shutdown().await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn release_controller(
        self: Rc<Self>,
        _params: proc_control::ReleaseControllerParams,
        mut results: proc_control::ReleaseControllerResults,
    ) -> capnp::Result<()> {
        let r = g3_daemon::control::quit::release_controller().await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_server(
        self: Rc<Self>,
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

    async fn get_server(
        self: Rc<Self>,
        params: proc_control::GetServerParams,
        mut results: proc_control::GetServerResults,
    ) -> capnp::Result<()> {
        let server = params.get()?.get_name()?.to_str()?;
        set_fetch_result::<server_control::Owned>(
            results.get().init_server(),
            super::server::ServerControlImpl::new_client(server),
        )
    }

    async fn publish_key(
        self: Rc<Self>,
        params: proc_control::PublishKeyParams,
        mut results: proc_control::PublishKeyResults,
    ) -> capnp::Result<()> {
        let pem = params.get()?.get_pem()?.to_str()?;
        let r = crate::control::bridge::add_key(pem).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn list_keys(
        self: Rc<Self>,
        _params: proc_control::ListKeysParams,
        mut results: proc_control::ListKeysResults,
    ) -> capnp::Result<()> {
        let r = crate::control::bridge::list_keys()
            .await
            .unwrap_or_default();
        let mut builder = results.get().init_result(r.len() as u32);
        for (i, ski) in r.iter().enumerate() {
            builder.set(i as u32, ski.as_slice());
        }
        Ok(())
    }

    async fn check_key(
        self: Rc<Self>,
        params: proc_control::CheckKeyParams,
        mut results: proc_control::CheckKeyResults,
    ) -> capnp::Result<()> {
        let ski = params.get()?.get_ski()?.to_vec();
        let r = crate::control::bridge::check_key(ski).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }

    async fn add_metrics_tag(
        self: Rc<Self>,
        params: proc_control::AddMetricsTagParams,
        mut results: proc_control::AddMetricsTagResults,
    ) -> capnp::Result<()> {
        let name = params.get()?.get_name()?.to_str()?;
        let value = params.get()?.get_value()?.to_str()?;

        let r = do_add_metrics_tag(name, value);
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }
}

fn do_add_metrics_tag(name: &str, value: &str) -> anyhow::Result<()> {
    let name =
        MetricTagName::from_str(name).map_err(|e| anyhow!("invalid metrics tag name: {e}"))?;
    let value =
        MetricTagValue::from_str(value).map_err(|e| anyhow!("invalid metrics tag value: {e}"))?;

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
