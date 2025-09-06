/*
 * SPDX-License-Identifier: Apache-2.0
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::Yaml;

pub(crate) fn load(v: &Yaml, _conf_dir: &Path) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => {
            crate::sticky::set_redis_url(Some(s));
            Ok(())
        }
        Yaml::Hash(map) => {
            let mut url: Option<String> = None;
            let mut prefix: Option<String> = None;
            let mut default_ttl: Option<std::time::Duration> = None;
            let mut max_ttl: Option<std::time::Duration> = None;
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "url" | "redis" => {
                    if let Yaml::String(s) = v {
                        url = Some(s.clone());
                        Ok(())
                    } else {
                        Err(anyhow!("invalid sticky redis url value"))
                    }
                }
                "prefix" => {
                    if let Yaml::String(s) = v {
                        prefix = Some(s.clone());
                        Ok(())
                    } else {
                        Err(anyhow!("invalid sticky prefix value"))
                    }
                }
                "default_ttl" | "default" => {
                    let d = g3_yaml::humanize::as_duration(v)
                        .map_err(|_| anyhow!("invalid sticky default_ttl value"))?;
                    default_ttl = Some(d);
                    Ok(())
                }
                "max_ttl" | "maximum_ttl" => {
                    let d = g3_yaml::humanize::as_duration(v)
                        .map_err(|_| anyhow!("invalid sticky max_ttl value"))?;
                    max_ttl = Some(d);
                    Ok(())
                }
                _ => Ok(()),
            })?;
            if let Some(u) = url.as_deref() {
                crate::sticky::set_redis_url(Some(u));
            }
            if let Some(p) = prefix.as_deref() {
                crate::sticky::set_prefix(Some(p));
            }
            if let Some(d) = default_ttl { crate::sticky::set_default_ttl(Some(d)); }
            if let Some(d) = max_ttl { crate::sticky::set_max_ttl(Some(d)); }
            Ok(())
        }
        _ => Err(anyhow!("invalid sticky config value")),
    }
}
