/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::collection::SelectivePickPolicy;

pub fn as_selective_pick_policy(value: &Yaml) -> anyhow::Result<SelectivePickPolicy> {
    if let Yaml::String(s) = value {
        let pick_policy =
            SelectivePickPolicy::from_str(s).map_err(|_| anyhow!("invalid pick policy"))?;
        Ok(pick_policy)
    } else {
        Err(anyhow!(
            "yaml value type for 'selective pick policy' should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_selective_pick_policy_ok() {
        // valid pick policy
        let value = yaml_str!("random");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::Random
        );

        let value = yaml_str!("serial");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::Serial
        );

        let value = yaml_str!("roundrobin");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::RoundRobin
        );

        let value = yaml_str!("ketama");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::Ketama
        );

        let value = yaml_str!("rendezvous");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::Rendezvous
        );

        let value = yaml_str!("jump");
        assert_eq!(
            as_selective_pick_policy(&value).unwrap(),
            SelectivePickPolicy::JumpHash
        )
    }

    #[test]
    fn as_selective_pick_policy_err() {
        // invalid pick policy
        let value = yaml_str!("invalid");
        assert!(as_selective_pick_policy(&value).is_err());

        let value = yaml_str!("");
        assert!(as_selective_pick_policy(&value).is_err());

        // non-string value
        let value = Yaml::Integer(1);
        assert!(as_selective_pick_policy(&value).is_err());

        let value = Yaml::Boolean(true);
        assert!(as_selective_pick_policy(&value).is_err());
    }
}
