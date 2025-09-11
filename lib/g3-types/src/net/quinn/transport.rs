/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use quinn::{IdleTimeout, TransportConfig, VarInt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuinnTransportConfigBuilder {
    max_idle_timeout: IdleTimeout,
    keep_alive_interval: Duration,
    stream_receive_window: Option<VarInt>,
    receive_window: Option<VarInt>,
    send_window: Option<u64>,
}

impl Default for QuinnTransportConfigBuilder {
    fn default() -> Self {
        QuinnTransportConfigBuilder {
            max_idle_timeout: IdleTimeout::from(VarInt::from_u32(60_000)), // 60s
            keep_alive_interval: Duration::from_secs(10),
            stream_receive_window: None,
            receive_window: None,
            send_window: None,
        }
    }
}

impl QuinnTransportConfigBuilder {
    pub fn set_max_idle_timeout(&mut self, timeout: Duration) -> anyhow::Result<()> {
        self.max_idle_timeout = IdleTimeout::try_from(timeout)?;
        Ok(())
    }

    pub fn set_keep_alive_interval(&mut self, interval: Duration) {
        self.keep_alive_interval = interval;
    }

    pub fn set_stream_receive_window(&mut self, size: u32) {
        self.stream_receive_window = Some(VarInt::from_u32(size));
    }

    pub fn set_receive_window(&mut self, size: u32) {
        self.receive_window = Some(VarInt::from_u32(size));
    }

    pub fn set_send_window(&mut self, size: u32) {
        self.send_window = Some(size as u64);
    }

    pub fn build_for_client(&self) -> TransportConfig {
        let mut config = TransportConfig::default();
        config
            .max_concurrent_bidi_streams(VarInt::from_u32(0))
            .max_concurrent_uni_streams(VarInt::from_u32(0))
            .max_idle_timeout(Some(self.max_idle_timeout))
            .keep_alive_interval(Some(self.keep_alive_interval));
        if let Some(v) = self.stream_receive_window {
            config.stream_receive_window(v);
        }
        if let Some(v) = self.receive_window {
            config.receive_window(v);
        }
        if let Some(v) = self.send_window {
            config.send_window(v);
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations() {
        let mut builder = QuinnTransportConfigBuilder::default();
        builder
            .set_max_idle_timeout(Duration::from_secs(120))
            .unwrap();
        builder.set_keep_alive_interval(Duration::from_secs(15));
        builder.set_stream_receive_window(65536);
        builder.set_receive_window(131072);
        builder.set_send_window(262144);
        let _config = builder.build_for_client();
        assert_eq!(
            builder.max_idle_timeout,
            IdleTimeout::from(VarInt::from_u32(120_000))
        );
        assert_eq!(builder.keep_alive_interval, Duration::from_secs(15));
        assert_eq!(builder.stream_receive_window, Some(VarInt::from_u32(65536)));
        assert_eq!(builder.receive_window, Some(VarInt::from_u32(131072)));
        assert_eq!(builder.send_window, Some(262144));
    }
}
