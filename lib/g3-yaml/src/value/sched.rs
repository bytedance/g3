/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_compat::CpuAffinity;

fn cpu_affinity_add_value(set: &mut CpuAffinity, v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => set
            .parse_add(s)
            .map_err(|e| anyhow!("invalid CPU ID(s) string value {s}: {e}")),
        Yaml::Integer(i) => {
            let n = usize::try_from(*i)
                .map_err(|e| anyhow!("invalid CPU ID integer value {}: {e}", *i))?;
            set.add_id(n)
                .map_err(|e| anyhow!("invalid CPU ID {n}: {e}"))
        }
        _ => Err(anyhow!("invalid yaml value type for CPU ID list")),
    }
}

pub fn as_cpu_set(v: &Yaml) -> anyhow::Result<CpuAffinity> {
    let mut set = CpuAffinity::default();

    if let Yaml::Array(seq) = v {
        for (i, v) in seq.iter().enumerate() {
            cpu_affinity_add_value(&mut set, v)
                .context(format!("invalid CPU ID list value for #{i}"))?;
        }
    } else {
        cpu_affinity_add_value(&mut set, v)?;
    }

    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_cpu_set_ok() {
        // valid array of CPU IDs
        let yaml = yaml_doc!(
            r#"
                - 0
                - 1
                - 2
            "#
        );
        assert_eq!(as_cpu_set(&yaml).unwrap().cpu_id_list(), &[0, 1, 2]);

        // valid single CPU ID
        let yaml = yaml_str!("3");
        assert_eq!(as_cpu_set(&yaml).unwrap().cpu_id_list(), &[3]);

        let yaml = Yaml::Integer(5);
        assert_eq!(as_cpu_set(&yaml).unwrap().cpu_id_list(), &[5]);

        // valid range of CPU IDs
        let yaml = yaml_doc!(
            r#"
                - 0-2
                - 4
            "#
        );
        assert_eq!(as_cpu_set(&yaml).unwrap().cpu_id_list(), &[0, 1, 2, 4]);
    }

    #[test]
    fn as_cpu_set_err() {
        // invalid string value
        let yaml = yaml_str!("abc");
        assert!(as_cpu_set(&yaml).is_err());

        // invalid integer value
        let yaml = Yaml::Integer(-1);
        assert!(as_cpu_set(&yaml).is_err());

        // invalid array value
        let yaml = yaml_doc!(
            r#"
                - 0
                - abc
            "#
        );
        assert!(as_cpu_set(&yaml).is_err());

        // invalid type
        let yaml = Yaml::Boolean(true);
        assert!(as_cpu_set(&yaml).is_err());
    }
}
