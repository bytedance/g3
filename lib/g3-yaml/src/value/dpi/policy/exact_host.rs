/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclExactHostRule;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclExactHostRule<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        let host = crate::value::as_host(value)?;
        self.add_host(host, action);
        Ok(())
    }
}

pub(super) fn as_exact_host_rule(
    value: &Yaml,
) -> anyhow::Result<AclExactHostRule<ProtocolInspectAction>> {
    let mut builder = AclExactHostRule::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod test {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use yaml_rust::YamlLoader;

    #[test]
    fn add_rule_for_action_ok() {
        let mut rule = AclExactHostRule::new(ProtocolInspectAction::Intercept);

        // valid ip
        let yaml = yaml_str!("192.168.1.1");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Bypass, &yaml)
                .is_ok()
        );

        // valid domain
        let yaml = yaml_str!("example.com");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Block, &yaml)
                .is_ok()
        );
    }

    #[test]
    fn add_rule_for_action_err() {
        let mut rule = AclExactHostRule::new(ProtocolInspectAction::Intercept);

        // invalid host
        let invalid_host_yaml = yaml_str!("invalid\u{e000}host!");
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Bypass, &invalid_host_yaml)
                .is_err()
        );

        // non-string YAML input
        let non_string_yaml = Yaml::Integer(123);
        assert!(
            rule.add_rule_for_action(ProtocolInspectAction::Bypass, &non_string_yaml)
                .is_err()
        );
    }

    #[test]
    fn as_exact_host_rule_ok() {
        let yaml = yaml_doc!(
            r#"
            block: "trusted.example.com"
            "#
        );
        let rule = as_exact_host_rule(&yaml).unwrap();
        let result = rule.check_domain("trusted.example.com");
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Block));

        let yaml = yaml_doc!(
            r#"
            intercept: "192.168.0.1"
            "#
        );
        let rule = as_exact_host_rule(&yaml).unwrap();
        let result = rule.check_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)));
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Intercept));

        let yaml = yaml_doc!(
            r#"
            bypass: "10.0.0.1"
            "#
        );
        let rule = as_exact_host_rule(&yaml).unwrap();
        let result = rule.check_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Bypass))
    }

    #[test]
    fn as_exact_host_rule_err() {
        // invalid key
        let yaml = yaml_doc!(
            r#"
            invalid_key: value
            "#
        );
        assert!(as_exact_host_rule(&yaml).is_err());

        // missing value
        let yaml = yaml_doc!(
            r#"
            block:
            "#
        );
        assert!(as_exact_host_rule(&yaml).is_err());

        // non-string input
        let yaml = Yaml::Boolean(true);
        assert!(as_exact_host_rule(&yaml).is_err())
    }
}
