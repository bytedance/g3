/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use crate::ext::OptionExt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TcpMiscSockOpts {
    pub no_delay: Option<bool>,
    pub max_segment_size: Option<u32>,
    pub time_to_live: Option<u32>,
    pub type_of_service: Option<u8>,
    pub netfilter_mark: Option<u32>,
}

impl TcpMiscSockOpts {
    #[must_use]
    pub fn adjust_to(self, other: &Self) -> Self {
        let no_delay = match (self.no_delay, other.no_delay) {
            (None, None) => None,
            (Some(true), _) | (_, Some(true)) => Some(true),
            _ => Some(false),
        };

        let max_segment_size = self.max_segment_size.existed_min(other.max_segment_size);
        let time_to_live = self.time_to_live.existed_min(other.time_to_live);

        let type_of_service = other.type_of_service.or(self.type_of_service);
        let netfilter_mark = other.netfilter_mark.or(self.netfilter_mark);

        TcpMiscSockOpts {
            no_delay,
            max_segment_size,
            time_to_live,
            type_of_service,
            netfilter_mark,
        }
    }
}
