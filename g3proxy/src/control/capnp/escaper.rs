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

use std::sync::Arc;

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::metrics::MetricsName;

use g3proxy_proto::escaper_capnp::escaper_control;

use super::set_operation_result;
use crate::escape::ArcEscaper;

pub(super) struct EscaperControlImpl {
    escaper: ArcEscaper,
}

impl EscaperControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<escaper_control::Client> {
        let name = unsafe { MetricsName::from_str_unchecked(name) };
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
        let data = pry!(pry!(params.get()).get_data()).to_string();
        let escaper = Arc::clone(&self.escaper);
        Promise::from_future(async move {
            set_operation_result(results.get().init_result(), escaper.publish(data).await);
            Ok(())
        })
    }
}
