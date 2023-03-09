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

use std::sync::Arc;

pub trait LimitedRecvStats {
    fn add_recv_bytes(&self, size: usize);
    fn add_recv_packet(&self);
}
pub type ArcLimitedRecvStats = Arc<dyn LimitedRecvStats + Send + Sync>;

pub trait LimitedSendStats {
    fn add_send_bytes(&self, size: usize);
    fn add_send_packet(&self);
}
pub type ArcLimitedSendStats = Arc<dyn LimitedSendStats + Send + Sync>;
