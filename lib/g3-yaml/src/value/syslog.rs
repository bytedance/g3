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

use std::convert::TryFrom;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_syslog::{SyslogBackendBuilder, SyslogBuilder, SyslogFormatterKind};

fn as_syslog_format_rfc5424(value: &Yaml) -> anyhow::Result<SyslogFormatterKind> {
    let mut enterprise_id = 0i32;
    let mut message_id: Option<String> = None;

    match value {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "enterprise_id" => {
                    enterprise_id =
                        crate::value::as_i32(v).context(format!("invalid value for key {k}"))?;
                    Ok(())
                }
                "message_id" => {
                    message_id = Some(
                        crate::value::as_string(v).context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
        }
        Yaml::Integer(i) => {
            enterprise_id = i32::try_from(*i).map_err(|e| anyhow!("invalid enterprise_id: {e}"))?;
            Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
        }
        Yaml::String(s) => {
            message_id = Some(s.to_string());
            Ok(SyslogFormatterKind::Rfc5424(enterprise_id, message_id))
        }
        _ => Err(anyhow!("invalid yaml value for rfc5424 syslog format")),
    }
}

fn as_syslog_backend_udp(value: &Yaml) -> anyhow::Result<SyslogBackendBuilder> {
    match value {
        Yaml::Hash(map) => {
            let mut addr: Option<SocketAddr> = None;
            let mut bind: Option<IpAddr> = None;

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "address" | "addr" => {
                    addr = Some(
                        crate::value::as_sockaddr(v)
                            .context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                "bind_ip" | "bind" => {
                    bind = Some(
                        crate::value::as_ipaddr(v).context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            if let Some(addr) = addr.take() {
                Ok(SyslogBackendBuilder::Udp(bind, addr))
            } else {
                Err(anyhow!("no target address has been set"))
            }
        }
        Yaml::String(s) => {
            let addr = SocketAddr::from_str(s).map_err(|e| anyhow!("invalid SocketAddr: {e}"))?;
            Ok(SyslogBackendBuilder::Udp(None, addr))
        }
        _ => Err(anyhow!("invalid yaml value for udp syslog backend")),
    }
}

fn as_syslog_backend_unix(value: &Yaml) -> anyhow::Result<SyslogBackendBuilder> {
    match value {
        Yaml::Hash(map) => {
            let mut path: Option<PathBuf> = None;

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "path" => {
                    path = Some(
                        crate::value::as_absolute_path(v)
                            .context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            if let Some(path) = path.take() {
                Ok(SyslogBackendBuilder::Unix(path))
            } else {
                Err(anyhow!("no path has been set"))
            }
        }
        Yaml::String(_) => {
            let path = crate::value::as_absolute_path(value)?;
            Ok(SyslogBackendBuilder::Unix(path))
        }
        _ => Err(anyhow!("invalid yaml value for unix syslog backend")),
    }
}

pub fn as_syslog_builder(value: &Yaml, ident: String) -> anyhow::Result<SyslogBuilder> {
    match value {
        Yaml::Hash(map) => {
            let mut builder = SyslogBuilder::with_ident(ident);
            let mut use_cee_log_syntax = false;
            let mut cee_event_flag: Option<String> = None;
            builder.set_facility(g3_syslog::Facility::Daemon);
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "target_unix" | "backend_unix" => {
                    let backend =
                        as_syslog_backend_unix(v).context(format!("invalid value for key {k}"))?;
                    builder.set_backend(backend);
                    Ok(())
                }
                "target_udp" | "backend_udp" => {
                    let backend =
                        as_syslog_backend_udp(v).context(format!("invalid value for key {k}"))?;
                    builder.set_backend(backend);
                    Ok(())
                }
                "target" | "backend" => {
                    if let Yaml::Hash(map) = v {
                        crate::hash::foreach_kv(map, |k, v| {
                            match crate::key::normalize(k).as_str() {
                                "udp" => {
                                    let backend = as_syslog_backend_udp(v)
                                        .context(format!("invalid value for key {k}"))?;
                                    builder.set_backend(backend);
                                    Ok(())
                                }
                                "unix" => {
                                    let backend = as_syslog_backend_unix(v)
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
                    let format = as_syslog_format_rfc5424(v)
                        .context(format!("invalid value for key {k}"))?;
                    builder.set_format(format);
                    Ok(())
                }
                "use_cee_log_syntax" | "use_cls" => {
                    use_cee_log_syntax = crate::value::as_bool(v)
                        .context(format!("invalid boolean value for key {k}"))?;
                    Ok(())
                }
                "cee_event_flag" | "cee_cookie" => {
                    let s = crate::value::as_ascii(v)
                        .context(format!("invalid ascii string value for key {k}"))?;
                    cee_event_flag = Some(s.to_string());
                    Ok(())
                }
                "emit_hostname" => {
                    let enable = crate::value::as_bool(v)
                        .context(format!("invalid boolean value for key {k}"))?;
                    builder.set_emit_hostname(enable);
                    Ok(())
                }
                "append_report_ts" => {
                    let enable = crate::value::as_bool(v)
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
            builder.set_facility(g3_syslog::Facility::Daemon);
            Ok(builder)
        }
        _ => Err(anyhow!(
            "yaml value type for 'SyslogBuilder' should be 'map'"
        )),
    }
}
