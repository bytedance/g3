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

use crate::ext::OptionExt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UdpMiscSockOpts {
    pub time_to_live: Option<u32>,
    pub type_of_service: Option<u8>,
    pub netfilter_mark: Option<u32>,
}

impl UdpMiscSockOpts {
    #[must_use]
    pub fn adjust_to(self, other: &Self) -> Self {
        let time_to_live = self.time_to_live.existed_min(other.time_to_live);

        let type_of_service = other.type_of_service.or(self.type_of_service);
        let netfilter_mark = other.netfilter_mark.or(self.netfilter_mark);

        UdpMiscSockOpts {
            time_to_live,
            type_of_service,
            netfilter_mark,
        }
    }
}
