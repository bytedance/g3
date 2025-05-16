/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{Host, UpstreamAddr};
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Default, Debug, Eq, PartialEq)]
pub(crate) struct SniHostConfig {
    redirect_host: Option<Host>,
    redirect_port: Option<u16>,
}

impl SniHostConfig {
    fn check(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    pub(crate) fn redirect(&self, orig_ups: &UpstreamAddr) -> UpstreamAddr {
        if let Some(host) = &self.redirect_host {
            let port = self.redirect_port.unwrap_or_else(|| orig_ups.port());
            UpstreamAddr::new(host.clone(), port)
        } else {
            let mut upstream = orig_ups.clone();
            if let Some(port) = self.redirect_port {
                upstream.set_port(port);
            }
            upstream
        }
    }
}

impl YamlMapCallback for SniHostConfig {
    fn type_name(&self) -> &'static str {
        "SniHostConfig"
    }

    #[inline]
    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        _doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(key).as_str() {
            "redirect_host" => {
                let host = g3_yaml::value::as_host(value)
                    .context(format!("invalid host value for key {key}"))?;
                self.redirect_host = Some(host);
                Ok(())
            }
            "redirect_port" => {
                let port = g3_yaml::value::as_u16(value)
                    .context(format!("invalid u16 value for key {key}"))?;
                self.redirect_port = Some(port);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {key}")),
        }
    }

    #[inline]
    fn check(&mut self) -> anyhow::Result<()> {
        self.check()
    }
}
