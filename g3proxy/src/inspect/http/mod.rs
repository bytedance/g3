/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod v2;
pub(super) use v2::{H2InterceptObject, H2InterceptionError};

mod v1;
pub(crate) use v1::H1InterceptObject;
pub(super) use v1::H1InterceptionError;
