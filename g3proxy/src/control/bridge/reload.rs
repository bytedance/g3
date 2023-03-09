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

use anyhow::anyhow;

use g3_yaml::YamlDocPosition;

macro_rules! impl_reload {
    ($f:ident, $m:tt) => {
        pub(in crate::control) async fn $f(
            name: String,
            position: Option<YamlDocPosition>,
        ) -> anyhow::Result<()> {
            g3_daemon::control::bridge::main_runtime_handle()
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
