/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use crate::IcapServiceClient;

mod error;
pub use error::IcapReqmodParseError;

mod payload;
use payload::IcapReqmodResponsePayload;

mod response;

pub mod h1;
pub mod h2;

pub mod mail;

pub mod imap;
pub mod smtp;

#[derive(Clone)]
pub struct IcapReqmodClient {
    inner: Arc<IcapServiceClient>,
}

impl IcapReqmodClient {
    pub fn new(inner: Arc<IcapServiceClient>) -> IcapReqmodClient {
        IcapReqmodClient { inner }
    }

    pub fn bypass(&self) -> bool {
        self.inner.config.bypass
    }
}
