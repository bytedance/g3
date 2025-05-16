/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Borrow;
use std::ops::Deref;

use smol_str::SmolStr;

#[derive(Clone)]
pub struct HttpOriginalHeaderName(SmolStr);

impl HttpOriginalHeaderName {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'a> From<&'a str> for HttpOriginalHeaderName {
    fn from(value: &'a str) -> Self {
        HttpOriginalHeaderName(value.into())
    }
}

impl Borrow<str> for HttpOriginalHeaderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for HttpOriginalHeaderName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
