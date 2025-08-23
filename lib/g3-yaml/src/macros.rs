/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

#[macro_export]
macro_rules! yaml_str {
    ($s:literal) => {
        Yaml::String($s.to_string())
    };
}

#[macro_export]
macro_rules! yaml_doc {
    ($s:literal) => {
        YamlLoader::load_from_str($s).unwrap().remove(0)
    };
}

#[cfg(test)]
mod tests {
    use yaml_rust::{Yaml, YamlLoader};

    #[test]
    fn yaml_str() {
        let v = yaml_str!("1234");
        assert!(matches!(v, Yaml::String(_)));
    }

    #[test]
    fn yaml_doc() {
        let v = yaml_doc!(
            r#"
                key1: v1
            "#
        );
        assert!(matches!(v, Yaml::Hash(_)));

        let v = yaml_doc!(
            r#"
                - v1
                - v2
            "#
        );
        assert!(matches!(v, Yaml::Array(_)));

        let v = yaml_doc!("1234");
        assert!(matches!(v, Yaml::Integer(1234)));

        let v = yaml_doc!("1.0");
        assert!(matches!(v, Yaml::Real(_)));

        let v = yaml_doc!(r#""1234""#);
        assert!(matches!(v, Yaml::String(_)));
    }
}
