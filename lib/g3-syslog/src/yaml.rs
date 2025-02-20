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

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::{Facility, SyslogBackendBuilder, SyslogBuilder, SyslogFormatterKind};

impl SyslogBuilder {
    pub fn parse_yaml(value: &Yaml, ident: &'static str) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut builder = SyslogBuilder::with_ident(ident);
                let mut use_cee_log_syntax = false;
                let mut cee_event_flag: Option<String> = None;
                builder.set_facility(Facility::Daemon);
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    #[cfg(unix)]
                    "target_unix" | "backend_unix" => {
                        let backend = SyslogBackendBuilder::parse_unix_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_backend(backend);
                        Ok(())
                    }
                    "target_udp" | "backend_udp" => {
                        let backend = SyslogBackendBuilder::parse_udp_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_backend(backend);
                        Ok(())
                    }
                    "target" | "backend" => {
                        if let Yaml::Hash(map) = v {
                            g3_yaml::foreach_kv(map, |k, v| {
                                match g3_yaml::key::normalize(k).as_str() {
                                    "udp" => {
                                        let backend = SyslogBackendBuilder::parse_udp_yaml(v)
                                            .context(format!("invalid value for key {k}"))?;
                                        builder.set_backend(backend);
                                        Ok(())
                                    }
                                    #[cfg(unix)]
                                    "unix" => {
                                        let backend = SyslogBackendBuilder::parse_unix_yaml(v)
                                            .context(format!("invalid value for key {k}"))?;
                                        builder.set_backend(backend);
                                        Ok(())
                                    }
                                    _ => Err(anyhow!("invalid key {k}")),
                                }
                            })
                            .context(format!("invalid value for key {k}"))
                        } else {
                            Err(anyhow!("yaml value type for key {k} should be 'map'"))
                        }
                    }
                    "format_rfc5424" => {
                        let format = SyslogFormatterKind::parse_rfc5424_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_format(format);
                        Ok(())
                    }
                    "use_cee_log_syntax" | "use_cls" => {
                        use_cee_log_syntax = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        Ok(())
                    }
                    "cee_event_flag" | "cee_cookie" => {
                        let s = g3_yaml::value::as_ascii(v)
                            .context(format!("invalid ascii string value for key {k}"))?;
                        cee_event_flag = Some(s.to_string());
                        Ok(())
                    }
                    "emit_hostname" => {
                        let enable = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        builder.set_emit_hostname(enable);
                        Ok(())
                    }
                    "append_report_ts" => {
                        let enable = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        builder.append_report_ts(enable);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
                if use_cee_log_syntax {
                    builder.enable_cee_log_syntax(cee_event_flag);
                }
                Ok(builder)
            }
            Yaml::Null => {
                let mut builder = SyslogBuilder::with_ident(ident);
                builder.set_facility(Facility::Daemon);
                Ok(builder)
            }
            _ => Err(anyhow!(
                "yaml value type for 'SyslogBuilder' should be 'map'"
            )),
        }
    }
}
