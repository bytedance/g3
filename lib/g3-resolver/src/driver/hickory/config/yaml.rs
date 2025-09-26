/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_socket::BindAddr;

use super::HickoryDriverConfig;

impl HickoryDriverConfig {
    pub fn set_by_yaml_kv(
        &mut self,
        k: &str,
        v: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "server" => match v {
                Yaml::String(addrs) => self.parse_server_str(addrs),
                Yaml::Array(seq) => self.parse_server_array(seq),
                _ => Err(anyhow!("invalid yaml value type, expect string / array")),
            },
            "server_port" => {
                let port = g3_yaml::value::as_u16(v)?;
                self.server_port = Some(port);
                Ok(())
            }
            "encryption" | "encrypt" => {
                let config = g3_yaml::value::as_dns_encryption_protocol_builder(v, lookup_dir)
                    .context(format!("invalid dns encryption config value for key {k}"))?;
                self.encryption = Some(config);
                Ok(())
            }
            "connect_timeout" => {
                self.connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "request_timeout" => {
                self.request_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "each_timeout" => {
                self.each_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "each_tries" | "retry_attempts" => {
                self.each_tries = g3_yaml::value::as_i32(v)?;
                Ok(())
            }
            "bind_ip" => {
                let ip = g3_yaml::value::as_ipaddr(v)?;
                self.bind_addr = BindAddr::Ip(ip);
                Ok(())
            }
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            "bind_interface" => {
                let interface = g3_yaml::value::as_interface(v)
                    .context(format!("invalid interface name value for key {k}"))?;
                self.bind_addr = BindAddr::Interface(interface);
                Ok(())
            }
            "tcp_misc_opts" => {
                self.tcp_misc_opts = g3_yaml::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "udp_misc_opts" => {
                self.udp_misc_opts = g3_yaml::value::as_udp_misc_sock_opts(v)
                    .context(format!("invalid udp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "positive_min_ttl" => {
                self.positive_min_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "positive_max_ttl" => {
                self.positive_max_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "negative_min_ttl" | "negative_ttl" => {
                self.negative_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "negative_max_ttl" => Ok(()),
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::DnsEncryptionProtocol;
    use g3_yaml::yaml_doc;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn set_by_yaml_kv_ok() {
        let mut config = HickoryDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: "8.8.8.8 8.8.4.4"
                server_port: 853
                encryption:
                  protocol: "dot"
                  tls_name: "dns.google"
                connect_timeout: "15s"
                request_timeout: "8s"
                each_timeout: "4s"
                each_tries: 3
                bind_ip: "127.0.0.1"
                tcp_misc_opts:
                  no_delay: true
                  max_segment_size: 1460
                udp_misc_opts:
                  time_to_live: 64
                positive_min_ttl: 300
                positive_max_ttl: 7200
                negative_min_ttl: 600
                negative_max_ttl: 1200
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v, None).unwrap();
        }

        assert_eq!(config.get_servers().len(), 2);
        assert_eq!(config.servers[0], IpAddr::from_str("8.8.8.8").unwrap());
        assert_eq!(config.servers[1], IpAddr::from_str("8.8.4.4").unwrap());
        assert_eq!(config.get_server_port(), Some(853));
        assert_eq!(
            config.get_encryption().unwrap().protocol(),
            DnsEncryptionProtocol::Tls
        );
        assert_eq!(config.connect_timeout, Duration::from_secs(15));
        assert_eq!(config.request_timeout, Duration::from_secs(8));
        assert_eq!(config.each_timeout, Duration::from_secs(4));
        assert_eq!(config.each_tries, 3);
        assert_eq!(
            config.get_bind_addr(),
            BindAddr::Ip(IpAddr::from_str("127.0.0.1").unwrap())
        );
        assert!(config.tcp_misc_opts.no_delay.unwrap());
        assert_eq!(config.tcp_misc_opts.max_segment_size, Some(1460));
        assert_eq!(config.udp_misc_opts.time_to_live, Some(64));
        assert_eq!(config.positive_min_ttl, 300);
        assert_eq!(config.positive_max_ttl, 7200);
        assert_eq!(config.negative_ttl, 600);

        // server as array
        let mut config = HickoryDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: ["1.1.1.1", "2001:db8::1"]
                server_port: 5353
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v, None).unwrap();
        }

        let mut expected = HickoryDriverConfig::default();
        expected.add_server_str("1.1.1.1").unwrap();
        expected.add_server_str("2001:db8::1").unwrap();
        expected.server_port = Some(5353);
        assert_eq!(config, expected);
    }

    #[test]
    fn set_by_yaml_kv_err() {
        let mut config = HickoryDriverConfig::default();
        let yaml = yaml_doc!(
            r#"
                server: 123
                server_port: -1
                encrypt:
                  protocol: "doh"
                connect_timeout: "invalid_duration"
                request_timeout: "8xs"
                each_timeout: "-4s"
                retry_attempts: 3.14
                bind_ip: "not.an.ip"
                tcp_misc_opts: "not_socket_opts"
                udp_misc_opts: 789
                positive_min_ttl: -300
                positive_max_ttl: "not_a_number"
                negative_ttl: false
                invalid_key: value
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v, None).is_err());
        }

        // invalid server array
        let yaml = yaml_doc!(
            r#"
                server: [456, "valid.server.com"]
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v, None).is_err());
        }

        // invalid server addresses
        let yaml = yaml_doc!(
            r#"
                server: "0.0.0.0 invalid-server 255.255.255.255"
            "#
        );
        for (k, v) in yaml.as_hash().unwrap().iter() {
            assert!(config.set_by_yaml_kv(k.as_str().unwrap(), v, None).is_err());
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    #[test]
    fn set_by_yaml_kv_bind_interface() {
        use g3_yaml::yaml_str;

        let mut config = HickoryDriverConfig::default();

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let interface_name = "lo";
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let interface_name = "lo0";

        let yaml = Yaml::String(interface_name.to_string());

        config
            .set_by_yaml_kv("bind_interface", &yaml, None)
            .unwrap();
        let interface = g3_yaml::value::as_interface(&yaml).unwrap();
        assert_eq!(config.bind_addr, BindAddr::Interface(interface));

        // invalid
        let yaml = Yaml::Integer(123);
        assert!(
            config
                .set_by_yaml_kv("bind_interface", &yaml, None)
                .is_err()
        );

        let yaml = yaml_str!("");
        assert!(
            config
                .set_by_yaml_kv("bind_interface", &yaml, None)
                .is_err()
        );
    }

    #[test]
    fn check_config() {
        let mut config = HickoryDriverConfig::default();
        assert!(config.check().is_err());

        let yaml = yaml_doc! {
            r#"
                server: "8.8.8.8"
                positive_min_ttl: 7200
                positive_max_ttl: 300
            "#
        };
        for (k, v) in yaml.as_hash().unwrap().iter() {
            config.set_by_yaml_kv(k.as_str().unwrap(), v, None).unwrap();
        }
        assert!(config.check().is_ok());
        assert_eq!(config.positive_min_ttl, 7200);
        assert_eq!(config.positive_max_ttl, 7200);
    }
}
