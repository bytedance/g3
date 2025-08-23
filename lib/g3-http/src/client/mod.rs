/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod error;
pub use error::HttpResponseParseError;

mod response;
pub use response::HttpForwardRemoteResponse;

mod transparent;
pub use transparent::HttpTransparentResponse;

mod adaptation;
pub use adaptation::HttpAdaptedResponse;
