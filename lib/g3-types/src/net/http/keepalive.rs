/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

const DEFAULT_HTTP_KEEPALIVE_IDLE: u64 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpKeepAliveConfig {
    enabled: bool,
    idle_expire: Duration,
}

impl Default for HttpKeepAliveConfig {
    fn default() -> Self {
        HttpKeepAliveConfig {
            enabled: true,
            idle_expire: Duration::from_secs(DEFAULT_HTTP_KEEPALIVE_IDLE),
        }
    }
}

impl HttpKeepAliveConfig {
    pub fn new(idle_expire: Duration) -> Self {
        HttpKeepAliveConfig {
            enabled: true,
            idle_expire,
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_idle_expire(&mut self, idle_expire: Duration) {
        self.idle_expire = idle_expire;
    }

    #[inline]
    pub fn idle_expire(&self) -> Duration {
        if self.enabled {
            self.idle_expire
        } else {
            Duration::ZERO
        }
    }

    #[must_use]
    pub fn adjust_to(self, other: Self) -> Self {
        let idle_expire = self.idle_expire.min(other.idle_expire);
        let enabled = self.enabled && other.enabled; // only if both enabled
        HttpKeepAliveConfig {
            enabled,
            idle_expire,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = HttpKeepAliveConfig::default();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(60));
    }

    #[test]
    fn creation_and_access() {
        let mut config = HttpKeepAliveConfig::new(Duration::from_secs(30));
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(30));

        config.set_enable(false);
        assert!(!config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::ZERO);

        config.set_idle_expire(Duration::from_secs(90));
        assert_eq!(config.idle_expire(), Duration::ZERO);
    }

    #[test]
    fn state_transitions() {
        let mut config = HttpKeepAliveConfig::default();
        config.set_enable(false);
        assert!(!config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::ZERO);

        config.set_enable(true);
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(60));
    }

    #[test]
    fn adjust_to_combinations() {
        // Both enabled
        let config_a = HttpKeepAliveConfig {
            enabled: true,
            idle_expire: Duration::from_secs(30),
        };
        let config_b = HttpKeepAliveConfig {
            enabled: true,
            idle_expire: Duration::from_secs(90),
        };
        let adjusted = config_a.adjust_to(config_b);
        assert!(adjusted.is_enabled());
        assert_eq!(adjusted.idle_expire, Duration::from_secs(30));

        // First disabled
        let config_c = HttpKeepAliveConfig {
            enabled: false,
            idle_expire: Duration::from_secs(30),
        };
        let adjusted = config_c.adjust_to(config_b);
        assert!(!adjusted.is_enabled());
        assert_eq!(adjusted.idle_expire, Duration::from_secs(30));

        // Second disabled
        let adjusted = config_b.adjust_to(config_c);
        assert!(!adjusted.is_enabled());
        assert_eq!(adjusted.idle_expire, Duration::from_secs(30));

        // Both disabled
        let config_d = HttpKeepAliveConfig {
            enabled: false,
            idle_expire: Duration::from_secs(90),
        };
        let adjusted = config_c.adjust_to(config_d);
        assert!(!adjusted.is_enabled());
        assert_eq!(adjusted.idle_expire, Duration::from_secs(30));
    }

    #[test]
    fn edge_cases() {
        let mut config = HttpKeepAliveConfig::new(Duration::ZERO);
        assert_eq!(config.idle_expire(), Duration::ZERO);

        config.set_idle_expire(Duration::MAX);
        assert_eq!(config.idle_expire(), Duration::MAX);
    }
}
