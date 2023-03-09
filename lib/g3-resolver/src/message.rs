/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use tokio::sync::oneshot;

use super::{ArcResolvedRecord, ResolvedRecord, ResolvedRecordSource, ResolverConfig};

#[derive(Clone, Debug)]
pub(crate) enum ResolverCommand {
    Quit,
    Update(Box<ResolverConfig>),
}

pub(crate) enum ResolveDriverRequest {
    GetV4(
        String,
        oneshot::Sender<(ArcResolvedRecord, ResolvedRecordSource)>,
    ),
    GetV6(
        String,
        oneshot::Sender<(ArcResolvedRecord, ResolvedRecordSource)>,
    ),
}

pub(crate) enum ResolveDriverResponse {
    V4(ResolvedRecord),
    V6(ResolvedRecord),
}
