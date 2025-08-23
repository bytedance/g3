/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::sync::oneshot;

use super::{ArcResolvedRecord, ResolvedRecord, ResolvedRecordSource, ResolverConfig};

#[derive(Clone, Debug)]
pub(crate) enum ResolverCommand {
    Quit,
    Update(Box<ResolverConfig>),
}

pub(crate) enum ResolveDriverRequest {
    GetV4(
        Arc<str>,
        oneshot::Sender<(ArcResolvedRecord, ResolvedRecordSource)>,
    ),
    GetV6(
        Arc<str>,
        oneshot::Sender<(ArcResolvedRecord, ResolvedRecordSource)>,
    ),
}

pub(crate) enum ResolveDriverResponse {
    V4(ResolvedRecord),
    V6(ResolvedRecord),
}
