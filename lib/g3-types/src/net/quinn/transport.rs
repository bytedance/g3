/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
