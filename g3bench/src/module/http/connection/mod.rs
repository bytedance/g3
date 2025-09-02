/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

mod h1;
pub(crate) use h1::{AppendH1ConnectArgs, H1ConnectArgs, SavedHttpForwardConnection};

mod h2;
pub(crate) use h2::{AppendH2ConnectArgs, H2ConnectArgs};

#[cfg(feature = "quic")]
mod h3;
#[cfg(feature = "quic")]
pub(crate) use h3::{AppendH3ConnectArgs, H3ConnectArgs};
