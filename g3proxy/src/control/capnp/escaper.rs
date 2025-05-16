/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::metrics::NodeName;

use g3proxy_proto::escaper_capnp::escaper_control;

use super::set_operation_result;
use crate::escape::ArcEscaper;

pub(super) struct EscaperControlImpl {
    escaper: ArcEscaper,
}

impl EscaperControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<escaper_control::Client> {
        let name = unsafe { NodeName::new_unchecked(name) };
        let escaper = crate::escape::get_escaper(&name)?;
        Ok(capnp_rpc::new_client(EscaperControlImpl { escaper }))
    }
}

impl escaper_control::Server for EscaperControlImpl {
    fn publish(
        &mut self,
        params: escaper_control::PublishParams,
        mut results: escaper_control::PublishResults,
    ) -> Promise<(), capnp::Error> {
        let data = pry!(pry!(pry!(params.get()).get_data()).to_string());
        let escaper = Arc::clone(&self.escaper);
        Promise::from_future(async move {
            set_operation_result(results.get().init_result(), escaper.publish(data).await);
            Ok(())
        })
    }
}
