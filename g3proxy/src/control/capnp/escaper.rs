/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::rc::Rc;

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
    async fn publish(
        self: Rc<Self>,
        params: escaper_control::PublishParams,
        mut results: escaper_control::PublishResults,
    ) -> capnp::Result<()> {
        let data = params.get()?.get_data()?.to_str()?;
        let r = self.escaper.publish(data).await;
        set_operation_result(results.get().init_result(), r);
        Ok(())
    }
}
