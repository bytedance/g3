/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_dpi::{MaybeProtocol, ProtocolPortMap};

fn as_maybe_protocol(value: &Yaml) -> anyhow::Result<Vec<MaybeProtocol>> {
    let mut r = Vec::new();

    match value {
        Yaml::String(s) => {
            let p = MaybeProtocol::from_str(s).map_err(|_| anyhow!("unrecognised protocol {s}"))?;
            r.push(p);
        }
        Yaml::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Yaml::String(s) = v {
                    let p = MaybeProtocol::from_str(s)
                        .map_err(|_| anyhow!("#{i}: unrecognised protocol {s}"))?;
                    r.push(p);
                } else {
                    return Err(anyhow!(
                        "the yaml value type for #{i} should be 'protocol string'"
                    ));
                }
            }
        }
        _ => return Err(anyhow!("invalid yaml value type")),
    }

    Ok(r)
}

fn update_by_map_value(portmap: &mut ProtocolPortMap, map: &yaml::Hash) -> anyhow::Result<()> {
    for (port, protocol) in map.iter() {
        let port = crate::value::as_u16(port)
            .context("the root map key should be valid u16 port value")?;
        let protocols = as_maybe_protocol(protocol)
            .context("the root map value should be valid protocol string(s) value")?;
        portmap.insert_batch(port, &protocols);
    }

    Ok(())
}

fn update_by_seq_value(portmap: &mut ProtocolPortMap, seq: &yaml::Array) -> anyhow::Result<()> {
    for (i, v) in seq.iter().enumerate() {
        if let Yaml::Hash(map) = v {
            let port = crate::hash_get_required(map, "port")?;
            let port =
                crate::value::as_u16(port).context("invalid u16 port value for key 'port'")?;
            let protocol = crate::hash_get_required(map, "protocol")?;
            let protocols = as_maybe_protocol(protocol)
                .context("invalid protocol string(s) value for key 'protocol'")?;
            portmap.insert_batch(port, &protocols);
        } else {
            return Err(anyhow!("the yaml value type for #{i} should be 'map'"));
        }
    }

    Ok(())
}

pub fn update_protocol_portmap(portmap: &mut ProtocolPortMap, value: &Yaml) -> anyhow::Result<()> {
    match value {
        Yaml::Hash(map) => update_by_map_value(portmap, map)
            .context("invalid yaml map value for 'protocol portmap'"),
        Yaml::Array(seq) => update_by_seq_value(portmap, seq)
            .context("invalid yaml seq value for 'protocol portmap'"),
        _ => Err(anyhow!("invalid yaml value type for 'protocol portmap'")),
    }
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn update_protocol_portmap_ok() {
        // map style value, single protocol.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            80: http
            25: smtp
            "#
        );
        update_protocol_portmap(&mut portmap, &yaml).unwrap();
        let mut expected = ProtocolPortMap::empty();
        expected.insert(80, MaybeProtocol::Http);
        expected.insert(25, MaybeProtocol::Smtp);
        assert_eq!(portmap, expected);

        // map style value, multiple protocols.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            443: [https, http]
            "#
        );
        update_protocol_portmap(&mut portmap, &yaml).unwrap();
        let mut expected = ProtocolPortMap::empty();
        expected.insert_batch(443, &[MaybeProtocol::Https, MaybeProtocol::Http]);
        assert_eq!(portmap, expected);

        // map style value, ssl only protocol.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            443: ssl
            "#
        );
        update_protocol_portmap(&mut portmap, &yaml).unwrap();
        let mut expected = ProtocolPortMap::empty();
        expected.insert(443, MaybeProtocol::Ssl);
        assert_eq!(portmap, expected);

        // sequence style value, single protocol.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            - port: 53
              protocol: dot
            - port: 22
              protocol: ssh
            "#
        );
        update_protocol_portmap(&mut portmap, &yaml).unwrap();
        let mut expected = ProtocolPortMap::empty();
        expected.insert(53, MaybeProtocol::DnsOverTls);
        expected.insert(22, MaybeProtocol::Ssh);
        assert_eq!(portmap, expected);

        // sequence style value, multiple protocols.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            - port: 993
              protocol: [imaps, imap]
            "#
        );
        update_protocol_portmap(&mut portmap, &yaml).unwrap();
        let mut expected = ProtocolPortMap::empty();
        expected.insert_batch(993, &[MaybeProtocol::Imaps, MaybeProtocol::Imap]);
        assert_eq!(portmap, expected);
    }

    #[test]
    fn update_protocol_portmap_err() {
        // invalid top-level type.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_str!("a string value");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // map style with invalid port key.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("key: http");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // map style with invalid protocol value type.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("80: 1024");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // map style with unrecognized protocol string.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("80: no-such-protocol");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // map style with invalid protocol value type in array.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("443: [https, 1024]");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // map style with unrecognized protocol string in array.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("443: [https, no-such-protocol]");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // seq style with invalid element type.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("- 1024");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // seq style with element missing 'port'.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("- protocol: http");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // seq style with element missing 'protocol'.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!("- port: 80");
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // seq style with invalid port value.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            - port: invalid
              protocol: http
            "#
        );
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());

        // seq style with invalid protocol value.
        let mut portmap = ProtocolPortMap::empty();
        let yaml = yaml_doc!(
            r#"
            - port: 80
              protocol: 1024
            "#
        );
        assert!(update_protocol_portmap(&mut portmap, &yaml).is_err());
    }
}
