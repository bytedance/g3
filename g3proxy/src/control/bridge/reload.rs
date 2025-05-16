/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

macro_rules! impl_reload {
    ($f:ident, $m:tt) => {
        pub(in crate::control) async fn $f(
            name: String,
            position: Option<YamlDocPosition>,
        ) -> anyhow::Result<()> {
            let name = unsafe { NodeName::new_unchecked(name) };
            g3_daemon::runtime::main_handle()
                .ok_or(anyhow!("unable to get main runtime handle"))?
                .spawn(async move { crate::$m::reload(&name, position).await })
                .await
                .map_err(|e| anyhow!("failed to spawn reload task: {e}"))?
        }
    };
}

impl_reload!(reload_user_group, auth);
impl_reload!(reload_auditor, audit);
impl_reload!(reload_resolver, resolve);
impl_reload!(reload_escaper, escape);
impl_reload!(reload_server, serve);
