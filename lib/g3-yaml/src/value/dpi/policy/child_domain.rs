/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclChildDomainRuleBuilder;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclChildDomainRuleBuilder<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        match value {
            Yaml::String(_) => {
                let host = crate::value::as_domain(value)?;
                self.add_node(&host, action);
                Ok(())
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub(super) fn as_child_domain_rule_builder(
    value: &Yaml,
) -> anyhow::Result<AclChildDomainRuleBuilder<ProtocolInspectAction>> {
    let mut builder = AclChildDomainRuleBuilder::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn add_rule_for_action_ok() {
        // valid domain addition
        let mut builder = AclChildDomainRuleBuilder::new(ProtocolInspectAction::Intercept);
        let yaml = yaml_str!("example.com");
        assert!(
            builder
                .add_rule_for_action(ProtocolInspectAction::Bypass, &yaml)
                .is_ok()
        );
    }

    #[test]
    fn add_rule_for_action_err() {
        // invalid domain format
        let mut builder = AclChildDomainRuleBuilder::new(ProtocolInspectAction::Intercept);
        let yaml = yaml_str!("invalid\u{e000}domain");
        assert!(
            builder
                .add_rule_for_action(ProtocolInspectAction::Intercept, &yaml)
                .is_err()
        );

        // non-string YAML type
        let mut builder = AclChildDomainRuleBuilder::new(ProtocolInspectAction::Intercept);
        let yaml = Yaml::Boolean(true);
        assert!(
            builder
                .add_rule_for_action(ProtocolInspectAction::Block, &yaml)
                .is_err()
        );
    }

    #[test]
    fn as_child_domain_rule_builder_ok() {
        let yaml = yaml_doc!(
            r#"
            intercept: "example.com"
            "#
        );
        let builder = as_child_domain_rule_builder(&yaml).unwrap();
        let rule = builder.build();

        let result = rule.check("example.com");
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Intercept));

        let result = rule.check("sub.example.com");
        assert!(result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Intercept));

        let result = rule.check("other.com");
        assert!(!result.0);
        assert!(matches!(result.1, ProtocolInspectAction::Intercept));
    }

    #[test]
    fn as_child_domain_rule_builder_err() {
        let yaml = yaml_doc!(
            r#"
            intercept:
            "#
        );
        assert!(as_child_domain_rule_builder(&yaml).is_err());

        let yaml = Yaml::Integer(42);
        assert!(as_child_domain_rule_builder(&yaml).is_err());
    }
}
