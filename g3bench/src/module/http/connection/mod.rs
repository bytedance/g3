/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod h1;
pub(crate) use h1::{
    AppendH1ConnectArgs, BoxHttpForwardReader, BoxHttpForwardWriter, H1ConnectArgs,
};

mod h2;

#[cfg(feature = "quic")]
mod h3;
