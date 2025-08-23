/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
