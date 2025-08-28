/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Borrow;
use std::ops::Deref;

use smol_str::SmolStr;

#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_original_header_name_operations() {
        let header_name = HttpOriginalHeaderName::from("Content-Type");
        assert_eq!(header_name.as_str(), "Content-Type");

        let header_name = HttpOriginalHeaderName::from("User-Agent");
        let s: &str = &header_name;
        assert_eq!(s, "User-Agent");

        let header_name = HttpOriginalHeaderName::from("Accept");
        let borrowed: &str = Borrow::<str>::borrow(&header_name);
        assert_eq!(borrowed, "Accept");
    }
}
