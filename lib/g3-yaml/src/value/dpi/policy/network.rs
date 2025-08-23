/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclNetworkRuleBuilder;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclNetworkRuleBuilder<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        let net = crate::value::as_ip_network(value)?;
        self.add_network(net, action);
        Ok(())
    }
}

pub(super) fn as_dst_subnet_rule_builder(
    value: &Yaml,
) -> anyhow::Result<AclNetworkRuleBuilder<ProtocolInspectAction>> {
    let mut builder = AclNetworkRuleBuilder::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use yaml_rust::YamlLoader;

    #[test]
    fn add_rule_for_action_ok() {
        let mut rule = AclNetworkRuleBuilder::new(ProtocolInspectAction::Intercept);

        // Valid IPv4 network
        let yaml = yaml_str!("192.168.1.0/24");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Bypass, &yaml)
                .is_ok()
        );

        // Valid IPv6 network
        let yaml = yaml_str!("2001:db8::/32");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Block, &yaml)
                .is_ok()
        );
    }

    #[test]
    fn add_rule_for_action_err() {
        let mut rule = AclNetworkRuleBuilder::new(ProtocolInspectAction::Intercept);

        // Invalid network format
        let yaml = yaml_str!("invalid\u{e000}network");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Bypass, &yaml)
                .is_err()
        );

        // Non-string YAML input
        let yaml = Yaml::Integer(42);
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Block, &yaml)
                .is_err()
        );
    }

    #[test]
    fn as_dst_subnet_rule_builder_ok() {
        let yaml = yaml_doc!(
            r#"
            intercept: "10.0.0.0/8"
            "#
        );
        let builder = as_dst_subnet_rule_builder(&yaml).unwrap();
        let rule = builder.build();
        let result = rule.check(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)));
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Intercept));

        let yaml = yaml_doc!(
            r#"
            bypass: "2001:db8::/32"
            "#
        );
        let builder = as_dst_subnet_rule_builder(&yaml).unwrap();
        let rule = builder.build();
        let result = rule.check(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Bypass));

        let yaml = yaml_doc!(
            r#"
            block: "192.168.0.0/16"
            "#
        );
        let builder = as_dst_subnet_rule_builder(&yaml).unwrap();
        let rule = builder.build();
        let result = rule.check(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)));
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Block));
    }

    #[test]
    fn as_dst_subnet_rule_builder_err() {
        // Invalid key
        let yaml = yaml_doc!(
            r#"
            invalid_key: "10.0.0.1/8"
            "#
        );
        assert!(as_dst_subnet_rule_builder(&yaml).is_err());

        // Missing value
        let yaml = yaml_doc!(
            r#"
            intercept:
            "#
        );
        assert!(as_dst_subnet_rule_builder(&yaml).is_err());

        // Non-string input
        let yaml = Yaml::Null;
        assert!(as_dst_subnet_rule_builder(&yaml).is_err());
    }
}
