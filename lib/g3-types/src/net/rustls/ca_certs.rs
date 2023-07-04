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
use rustls::Certificate;

pub fn load_native_certs_for_rustls() -> anyhow::Result<Vec<Certificate>> {
    rustls_native_certs::load_native_certs()
        .map(|certs| certs.into_iter().map(|v| Certificate(v.0)).collect())
        .map_err(|e| anyhow!("failed to load native ca certs: {e}"))
}
