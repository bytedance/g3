/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::time::{Instant, Interval};

#[derive(Default)]
pub struct OptionalInterval {
    inner: Option<Interval>,
}

impl OptionalInterval {
    pub fn with(inner: Interval) -> Self {
        OptionalInterval { inner: Some(inner) }
    }

    pub async fn tick(&mut self) -> Instant {
        match &mut self.inner {
            Some(interval) => interval.tick().await,
            None => std::future::pending().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn never() {
        let mut f = OptionalInterval::default();
        let r = tokio::time::timeout(Duration::from_millis(10), f.tick()).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn normal() {
        let interval = Duration::from_millis(8);
        let mut f = OptionalInterval::with(tokio::time::interval_at(
            Instant::now() + interval,
            interval,
        ));
        let r = tokio::time::timeout(Duration::from_millis(10), f.tick()).await;
        assert!(r.is_ok());
    }
}
