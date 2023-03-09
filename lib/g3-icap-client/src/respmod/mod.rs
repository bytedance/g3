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

use crate::IcapServiceClient;

mod error;
pub use error::IcapRespmodParseError;

mod payload;
pub use payload::IcapRespmodResponsePayload;

mod response;

pub mod h1;
pub mod h2;

#[derive(Clone)]
pub struct IcapRespmodClient {
    inner: Arc<IcapServiceClient>,
}

impl IcapRespmodClient {
    pub fn new(inner: Arc<IcapServiceClient>) -> IcapRespmodClient {
        IcapRespmodClient { inner }
    }

    pub fn bypass(&self) -> bool {
        self.inner.config.bypass
    }
}
