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

use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use yaml_rust::Yaml;

pub(crate) fn load(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = v {
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "country" => {
                let path = g3_yaml::value::as_file_path(v, conf_dir, false)?;
                let db = g3_geoip::vendor::native::load_country(&path)?;
                g3_geoip::store::store_country(Arc::new(db));
                Ok(())
            }
            "asn" => {
                let path = g3_yaml::value::as_file_path(v, conf_dir, false)?;
                let db = g3_geoip::vendor::native::load_asn(&path)?;
                g3_geoip::store::store_asn(Arc::new(db));
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!("invalid value type"))
    }
}
