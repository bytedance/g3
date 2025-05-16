/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod error;
pub use error::HttpRequestParseError;

mod request;
pub use request::HttpProxyClientRequest;

mod transparent;
pub use transparent::{HttpTransparentRequest, HttpTransparentRequestAcceptor};

mod adaptation;
pub use adaptation::HttpAdaptedRequest;

mod uri;
pub use uri::UriExt;
