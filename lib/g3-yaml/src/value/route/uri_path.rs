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

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::route::UriPathMatch;

use crate::{YamlDocPosition, YamlMapCallback};

fn add_url_path_matched_value<T: YamlMapCallback>(
    obj: &mut UriPathMatch<Arc<T>>,
    value: &Yaml,
    mut target: T,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Yaml::Hash(map) = value {
        let mut prefix_match_vs = vec![];
        let mut set_default = false;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "prefix_match" => {
                let prefix = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                prefix_match_vs.push(prefix);
                Ok(())
            }
            normalized_key => target
                .parse_kv(normalized_key, v, doc)
                .context(format!("failed to parse {type_name} value for key {k}")),
        })?;

        target
            .check()
            .context(format!("{type_name} final check failed"))?;

        let t = Arc::new(target);
        let mut auto_default = true;
        for prefix in &prefix_match_vs {
            if obj.add_prefix(prefix.to_string(), Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for path prefix {prefix}"
                ));
            }
            auto_default = false;
        }
        if (set_default || auto_default) && obj.set_default(t).is_some() {
            return Err(anyhow!("a default {type_name} value has already been set"));
        }

        Ok(())
    } else {
        Err(anyhow!(
            "yaml type for 'url path matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_url_path_matched_obj<T>(
    value: &Yaml,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<UriPathMatch<Arc<T>>>
where
    T: Default + YamlMapCallback,
{
    let mut obj = UriPathMatch::<Arc<T>>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_url_path_matched_value(&mut obj, v, target, doc).context(format!(
                "invalid url path matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_url_path_matched_value(&mut obj, value, target, doc)
            .context(format!("invalid url path matched {type_name} value"))?;
    }

    Ok(obj)
}
