/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_std_ext::core::OptionExt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UdpMiscSockOpts {
    pub time_to_live: Option<u32>,
    pub hop_limit: Option<u32>,
    pub type_of_service: Option<u8>,
    #[cfg(not(windows))]
    pub traffic_class: Option<u8>,
    #[cfg(target_os = "linux")]
    pub netfilter_mark: Option<u32>,
}

impl UdpMiscSockOpts {
    #[must_use]
    pub fn adjust_to(self, other: &Self) -> Self {
        UdpMiscSockOpts {
            time_to_live: self.time_to_live.existed_min(other.time_to_live),
            hop_limit: self.hop_limit.existed_min(other.hop_limit),
            type_of_service: other.type_of_service.or(self.type_of_service),
            #[cfg(not(windows))]
            traffic_class: other.traffic_class.or(self.traffic_class),
            #[cfg(target_os = "linux")]
            netfilter_mark: other.netfilter_mark.or(self.netfilter_mark),
        }
    }
}
