/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod close;
use close::{ClientCloseFrame, ServerCloseFrame};

mod h1;
pub(crate) use h1::H1WebsocketInterceptObject;

mod h2;
pub(crate) use self::h2::H2WebsocketInterceptObject;
