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
use arcstr::ArcStr;

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
    congestion_control: Option<ArcStr>,
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
        self.congestion_control = Some(ca.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_to_no_delay_logic() {
        let config1 = TcpMiscSockOpts {
            no_delay: None,
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            no_delay: None,
            ..Default::default()
        };
        let result = config1.adjust_to(&config2);
        assert_eq!(result.no_delay, None);

        let config1 = TcpMiscSockOpts {
            no_delay: Some(true),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            no_delay: Some(false),
            ..Default::default()
        };
        let result = config1.adjust_to(&config2);
        assert_eq!(result.no_delay, Some(true));

        let config1 = TcpMiscSockOpts {
            no_delay: Some(false),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            no_delay: Some(true),
            ..Default::default()
        };
        let result = config1.adjust_to(&config2);
        assert_eq!(result.no_delay, Some(true));

        let config1 = TcpMiscSockOpts {
            no_delay: Some(false),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            no_delay: Some(false),
            ..Default::default()
        };
        let result = config1.adjust_to(&config2);
        assert_eq!(result.no_delay, Some(false));
    }

    #[test]
    fn adjust_to_min_value_fields() {
        let config1 = TcpMiscSockOpts {
            max_segment_size: Some(1460),
            time_to_live: Some(64),
            hop_limit: Some(128),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            max_segment_size: Some(1200),
            time_to_live: Some(32),
            hop_limit: Some(64),
            ..Default::default()
        };

        let result = config1.adjust_to(&config2);
        assert_eq!(result.max_segment_size, Some(1200)); // min(1460, 1200)
        assert_eq!(result.time_to_live, Some(32)); // min(64, 32)
        assert_eq!(result.hop_limit, Some(64)); // min(128, 64)
    }

    #[test]
    fn adjust_to_min_with_none_values() {
        let config1 = TcpMiscSockOpts {
            max_segment_size: Some(1460),
            time_to_live: None,
            hop_limit: Some(64),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            max_segment_size: None,
            time_to_live: Some(32),
            hop_limit: None,
            ..Default::default()
        };

        let result = config1.adjust_to(&config2);
        assert_eq!(result.max_segment_size, Some(1460)); // existed_min(Some(1460), None)
        assert_eq!(result.time_to_live, Some(32)); // existed_min(None, Some(32))
        assert_eq!(result.hop_limit, Some(64)); // existed_min(Some(64), None)
    }

    #[test]
    fn adjust_to_override_fields() {
        let config1 = TcpMiscSockOpts {
            type_of_service: Some(0x10),
            #[cfg(not(windows))]
            traffic_class: Some(0x20),
            #[cfg(target_os = "linux")]
            netfilter_mark: Some(0x1000),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            type_of_service: Some(0x08),
            #[cfg(not(windows))]
            traffic_class: Some(0x40),
            #[cfg(target_os = "linux")]
            netfilter_mark: Some(0x2000),
            ..Default::default()
        };

        let result = config1.adjust_to(&config2);
        assert_eq!(result.type_of_service, Some(0x08)); // other takes precedence
        #[cfg(not(windows))]
        assert_eq!(result.traffic_class, Some(0x40)); // other takes precedence
        #[cfg(target_os = "linux")]
        assert_eq!(result.netfilter_mark, Some(0x2000)); // other takes precedence
    }

    #[test]
    fn adjust_to_override_with_none() {
        let config1 = TcpMiscSockOpts {
            type_of_service: Some(0x10),
            #[cfg(not(windows))]
            traffic_class: Some(0x20),
            #[cfg(target_os = "linux")]
            netfilter_mark: Some(0x1000),
            ..Default::default()
        };
        let config2 = TcpMiscSockOpts {
            type_of_service: None,
            #[cfg(not(windows))]
            traffic_class: None,
            #[cfg(target_os = "linux")]
            netfilter_mark: None,
            ..Default::default()
        };

        let result = config1.adjust_to(&config2);
        assert_eq!(result.type_of_service, Some(0x10)); // falls back to self
        #[cfg(not(windows))]
        assert_eq!(result.traffic_class, Some(0x20)); // falls back to self
        #[cfg(target_os = "linux")]
        assert_eq!(result.netfilter_mark, Some(0x1000)); // falls back to self
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "solaris",
        target_os = "illumos"
    ))]
    #[test]
    fn adjust_to_congestion_control() {
        let mut config1 = TcpMiscSockOpts::default();
        config1.set_congestion_control("cubic".to_string());
        let mut config2 = TcpMiscSockOpts::default();

        let result = config1.adjust_to(&config2);
        assert_eq!(result.congestion_control(), Some("cubic".as_bytes()));

        config2.set_congestion_control("bbr".to_string());
        let result = config1.adjust_to(&config2);
        assert_eq!(result.congestion_control(), Some("bbr".as_bytes()));
    }

    #[test]
    fn comprehensive_adjust_to() {
        let config1 = TcpMiscSockOpts {
            no_delay: Some(false),
            max_segment_size: Some(1500),
            time_to_live: Some(255),
            hop_limit: Some(255),
            type_of_service: None,
            #[cfg(not(windows))]
            traffic_class: Some(0x80),
            #[cfg(any(
                target_os = "linux",
                target_os = "freebsd",
                target_os = "solaris",
                target_os = "illumos"
            ))]
            congestion_control: Some(arcstr::literal!("reno")),
            #[cfg(target_os = "linux")]
            netfilter_mark: None,
        };

        let config2 = TcpMiscSockOpts {
            no_delay: Some(true),         // should win (true always wins)
            max_segment_size: Some(1200), // should win (smaller value)
            time_to_live: Some(128),      // should win (smaller value)
            hop_limit: Some(64),          // should win (smaller value)
            type_of_service: Some(0x04),  // should win (other takes precedence)
            #[cfg(not(windows))]
            traffic_class: None, // config1 value should remain
            #[cfg(any(
                target_os = "linux",
                target_os = "freebsd",
                target_os = "solaris",
                target_os = "illumos"
            ))]
            congestion_control: Some(arcstr::literal!("cubic")), // should win (other takes precedence)
            #[cfg(target_os = "linux")]
            netfilter_mark: Some(0x5678), // should win (other takes precedence)
        };

        let result = config1.adjust_to(&config2);

        assert_eq!(result.no_delay, Some(true));
        assert_eq!(result.max_segment_size, Some(1200));
        assert_eq!(result.time_to_live, Some(128));
        assert_eq!(result.hop_limit, Some(64));
        assert_eq!(result.type_of_service, Some(0x04));
        #[cfg(not(windows))]
        assert_eq!(result.traffic_class, Some(0x80));
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "solaris",
            target_os = "illumos"
        ))]
        assert_eq!(result.congestion_control(), Some("cubic".as_bytes()));
        #[cfg(target_os = "linux")]
        assert_eq!(result.netfilter_mark, Some(0x5678));
    }
}
