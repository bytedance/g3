/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use yaml_rust::Yaml;

use super::CAresDriverConfig;

impl CAresDriverConfig {
    pub fn set_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "server" => match v {
                Yaml::String(addrs) => self.parse_server_str(addrs),
                Yaml::Array(seq) => self.parse_server_array(seq),
                _ => Err(anyhow!("invalid yaml value type, expect string / array")),
            },
            "each_timeout" => {
                self.each_timeout = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "each_tries" => {
                self.each_tries = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "max_timeout" => {
                self.set_max_timeout(g3_yaml::value::as_i32(v)?);
                Ok(())
            }
            "udp_max_quires" => {
                self.set_udp_max_queries(g3_yaml::value::as_i32(v)?);
                Ok(())
            }
            "round_robin" => {
                self.round_robin = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "socket_send_buffer_size" => {
                let buf_size = g3_yaml::value::as_u32(v)?;
                self.so_send_buf_size = Some(buf_size);
                Ok(())
            }
            "socket_recv_buffer_size" => {
                let buf_size = g3_yaml::value::as_u32(v)?;
                self.so_recv_buf_size = Some(buf_size);
                Ok(())
            }
            "bind_ipv4" => {
                let ip4 = g3_yaml::value::as_ipv4addr(v)?;
                self.bind_v4 = Some(ip4);
                Ok(())
            }
            "bind_ipv6" => {
                let ip6 = g3_yaml::value::as_ipv6addr(v)?;
                self.bind_v6 = Some(ip6);
                Ok(())
            }
            "negative_min_ttl" | "negative_ttl" | "protective_cache_ttl" => {
                self.negative_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "positive_min_ttl" => {
                self.positive_min_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "positive_max_ttl" | "positive_ttl" | "max_cache_ttl" | "maximum_cache_ttl" => {
                self.positive_max_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "positive_del_ttl" => {
                self.positive_del_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
