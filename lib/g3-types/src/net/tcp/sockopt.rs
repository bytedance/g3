/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "solaris",
    target_os = "illumos"
))]
use std::sync::Arc;

use g3_std_ext::core::OptionExt;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TcpMiscSockOpts {
    pub no_delay: Option<bool>,
    pub max_segment_size: Option<u32>,
    pub time_to_live: Option<u32>,
    pub hop_limit: Option<u32>,
    pub type_of_service: Option<u8>,
    #[cfg(not(windows))]
    pub traffic_class: Option<u8>,
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "solaris",
        target_os = "illumos"
    ))]
    congestion_control: Option<Arc<str>>,
    #[cfg(target_os = "linux")]
    pub netfilter_mark: Option<u32>,
}

impl TcpMiscSockOpts {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "solaris",
        target_os = "illumos"
    ))]
    pub fn set_congestion_control(&mut self, ca: String) {
        self.congestion_control = Some(Arc::from(ca));
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "solaris",
        target_os = "illumos"
    ))]
    pub fn congestion_control(&self) -> Option<&[u8]> {
        self.congestion_control.as_ref().map(|v| v.as_bytes())
    }

    #[must_use]
    pub fn adjust_to(&self, other: &Self) -> Self {
        let no_delay = match (self.no_delay, other.no_delay) {
            (None, None) => None,
            (Some(true), _) | (_, Some(true)) => Some(true),
            _ => Some(false),
        };

        TcpMiscSockOpts {
            no_delay,
            max_segment_size: self.max_segment_size.existed_min(other.max_segment_size),
            time_to_live: self.time_to_live.existed_min(other.time_to_live),
            hop_limit: self.hop_limit.existed_min(other.hop_limit),
            type_of_service: other.type_of_service.or(self.type_of_service),
            #[cfg(not(windows))]
            traffic_class: other.traffic_class.or(self.traffic_class),
            #[cfg(any(
                target_os = "linux",
                target_os = "freebsd",
                target_os = "solaris",
                target_os = "illumos"
            ))]
            congestion_control: other
                .congestion_control
                .clone()
                .or(self.congestion_control.clone()),
            #[cfg(target_os = "linux")]
            netfilter_mark: other.netfilter_mark.or(self.netfilter_mark),
        }
    }
}
