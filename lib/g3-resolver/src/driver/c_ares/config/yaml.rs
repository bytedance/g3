/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;
    use yaml_rust::YamlLoader;

    #[test]
    fn set_by_yaml_kv_ok() {
        let mut config = CAresDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: "8.8.8.8 8.8.4.4"
                each_timeout: 3000
                each_tries: 2
                max_timeout: 5000
                udp_max_quires: 100
                round_robin: true
                socket_send_buffer_size: 8192
                socket_recv_buffer_size: 4096
                bind_ipv4: "192.168.1.1"
                bind_ipv6: "2001:db8::1"
                negative_min_ttl: 60
                positive_min_ttl: 300
                positive_max_ttl: 3600
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v).unwrap();
        }

        let mut expected = CAresDriverConfig::default();
        expected.add_server("8.8.8.8").unwrap();
        expected.add_server("8.8.4.4").unwrap();
        expected.each_timeout = 3000;
        expected.each_tries = 2;
        #[cfg(cares1_22)]
        {
            expected.max_timeout = 5000;
        }
        #[cfg(cares1_20)]
        {
            expected.udp_max_queries = 100;
        }
        expected.round_robin = true;
        expected.so_send_buf_size = Some(8192);
        expected.so_recv_buf_size = Some(4096);
        expected.bind_v4 = Some(Ipv4Addr::from_str("192.168.1.1").unwrap());
        expected.bind_v6 = Some(Ipv6Addr::from_str("2001:db8::1").unwrap());
        expected.negative_ttl = 60;
        expected.positive_min_ttl = 300;
        expected.positive_max_ttl = 3600;

        assert_eq!(config, expected);

        // server as array
        let mut config = CAresDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: ["1.1.1.1", "1.0.0.1:53"]
                each_timeout: 1500
                round_robin: false
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v).unwrap();
        }

        let mut expected = CAresDriverConfig::default();
        expected.add_server("1.1.1.1").unwrap();
        expected.add_server("1.0.0.1:53").unwrap();
        expected.each_timeout = 1500;
        expected.round_robin = false;

        assert_eq!(config, expected);

        // alternative key names
        let mut config = CAresDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                max_cache_ttl: 86400
                maximum_cache_ttl: 172800
                protective_cache_ttl: 180
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v).unwrap();
        }

        assert_eq!(config.negative_ttl, 180);
        assert_eq!(config.positive_max_ttl, 172800);
    }

    #[test]
    fn set_by_yaml_kv_err() {
        let mut config = CAresDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: 123
                each_timeout: "invalid"
                each_tries: -10
                max_timeout: "not_a_number"
                udp_max_quires: null
                round_robin: "not_a_boolean"
                socket_send_buffer_size: -1
                socket_recv_buffer_size: "invalid_size"
                bind_ipv4: "invalid_ip"
                bind_ipv6: "invalid_ipv6"
                negative_ttl: false
                positive_min_ttl: "invalid_ttl"
                positive_ttl: -100
                invalid_key: "value"
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v).is_err());
        }

        // invalid server array
        let yaml = yaml_doc!(
            r#"
                server: [456, "valid.server.com"]
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v).is_err());
        }

        // invalid server addresses
        let yaml = yaml_doc!(
            r#"
                server: "0.0.0.0 invalid-server 255.255.255.255"
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v).is_err());
        }
    }

    #[test]
    fn check_config() {
        let mut config = CAresDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                positive_min_ttl: 600
                positive_max_ttl: 300
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v).unwrap();
        }
        assert!(config.check().is_ok());
        assert_eq!(config.positive_max_ttl, 600);
    }
}
