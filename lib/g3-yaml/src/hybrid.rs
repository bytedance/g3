/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use super::YamlDocPosition;

pub struct HybridParser {
    conf_dir: PathBuf,
    conf_extension: Option<OsString>,
}

impl HybridParser {
    pub fn new(conf_dir: &Path, conf_extension: Option<&OsStr>) -> Self {
        HybridParser {
            conf_dir: PathBuf::from(conf_dir),
            conf_extension: conf_extension.map(|v| v.to_os_string()),
        }
    }

    pub fn foreach_map<F>(&self, value: &Yaml, f: F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        match value {
            Yaml::String(path) => self.load_path(path, &f).context(format!(
                "value is a string {path}, which should be a valid path",
            ))?,
            Yaml::Array(seq) => self
                .load_array(seq, &f)
                .context(format!("value is an array, with {} objects", seq.len()))?,
            _ => return Err(anyhow!("value should be a path or an array")),
        }
        Ok(())
    }

    fn get_final_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
        let mut final_path = self.conf_dir.clone();
        final_path.push(path);
        Ok(final_path.canonicalize()?)
    }

    fn load_array<F>(&self, entries: &[Yaml], f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        for (i, entry) in entries.iter().enumerate() {
            match entry {
                Yaml::String(path) => self
                    .load_path(path, f)
                    .context(format!("#{i}: failed to load path {path}"))?,
                Yaml::Hash(value) => {
                    f(value, None).context(format!("#{i}: failed to load map {value:?}"))?
                }
                _ => return Err(anyhow!("#{i}: value should be a path or a map")),
            }
        }
        Ok(())
    }

    fn load_path<F>(&self, path: &str, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        let path = self.get_final_path(path)?;
        if path.is_dir() {
            // NOTE symlink is followed
            self.load_dir(&path, f).context(format!(
                "failed to load conf from directory {}",
                path.display()
            ))?;
        } else if path.is_file() {
            // NOTE symlink is followed
            self.load_file(&path, f)
                .context(format!("failed to load conf from file {}", path.display()))?;
        } else {
            return Err(anyhow!(
                "path {} should be a directory or file",
                path.display()
            ));
        }
        Ok(())
    }

    fn load_dir<F>(&self, path: &Path, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        for d_entry in std::fs::read_dir(path)? {
            let d_entry = d_entry?;

            let file_name = d_entry.path();
            if let Some(conf_extension) = &self.conf_extension {
                let extension = match file_name.extension() {
                    Some(ext) => ext,
                    None => continue,
                };
                if extension != conf_extension {
                    continue;
                }
            }

            // this get the real file type, no following
            let file_type = d_entry.file_type()?;
            if file_type.is_file() {
                self.load_file(&file_name, f).context(format!(
                    "failed to load conf from file {}",
                    file_name.display()
                ))?;
            } else if file_type.is_symlink() {
                let real_file = file_name.canonicalize()?;
                if !real_file.is_file() {
                    continue;
                }

                self.load_file(&real_file, f).context(format!(
                    "failed to load conf from file {}, followed via symlink {}",
                    real_file.display(),
                    file_name.display()
                ))?;
            }
        }
        Ok(())
    }

    fn load_file<F>(&self, path: &Path, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        super::foreach_doc(path, |i, doc| match doc {
            Yaml::Hash(value) => {
                let position = YamlDocPosition {
                    path: PathBuf::from(path),
                    index: i,
                };
                f(value, Some(position)).context(format!(
                    "failed to load map in conf file {} doc {}",
                    path.display(),
                    i
                ))
            }
            _ => Err(anyhow!("doc {i} in {} should be a map", path.display())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use yaml_rust::yaml::Hash;

    /// Helper function to create a temporary test directory with unique naming
    fn create_test_dir() -> PathBuf {
        let temp_dir = std::env::temp_dir().join(format!(
            "g3_yaml_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        fs::create_dir_all(&temp_dir).unwrap();
        temp_dir
    }

    /// Helper function to create test file with given content
    fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let file_path = dir.join(name);
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file_path
    }

    #[test]
    fn new() {
        // With extension
        let conf_dir = Path::new("/test/path");
        let extension = Some(OsStr::new("yaml"));
        let parser = HybridParser::new(conf_dir, extension);
        assert_eq!(parser.conf_dir, PathBuf::from("/test/path"));
        assert_eq!(parser.conf_extension, Some(OsString::from("yaml")));

        // Without extension
        let parser = HybridParser::new(conf_dir, None);
        assert_eq!(parser.conf_dir, PathBuf::from("/test/path"));
        assert_eq!(parser.conf_extension, None);
    }

    #[test]
    fn foreach_map() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, Some(OsStr::new("yaml")));

        // String path
        create_test_file(&test_dir, "test.yaml", "key1: value1\nkey2: value2\n");
        let yaml_path = Yaml::String("test.yaml".to_string());
        let result = parser.foreach_map(&yaml_path, |hash, position| {
            assert!(hash.contains_key(&Yaml::String("key1".to_string())));
            assert!(position.is_some());
            Ok(())
        });
        assert!(result.is_ok());

        // Array value
        let mut direct_hash = Hash::new();
        direct_hash.insert(
            Yaml::String("direct".to_string()),
            Yaml::String("value".to_string()),
        );
        let yaml_array = Yaml::Array(vec![
            Yaml::String("test.yaml".to_string()),
            Yaml::Hash(direct_hash),
        ]);
        let result = parser.foreach_map(&yaml_array, |_, _| Ok(()));
        assert!(result.is_ok());

        // Invalid value type
        let yaml_int = Yaml::Integer(42);
        let result = parser.foreach_map(&yaml_int, |_, _| Ok(()));
        assert!(result.is_err());
    }

    #[test]
    fn get_final_path() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, None);

        // Absolute path
        let absolute_path = test_dir.to_string_lossy();
        let result = parser.get_final_path(&absolute_path).unwrap();
        assert_eq!(result, PathBuf::from(absolute_path.as_ref()));

        // Relative path
        create_test_file(&test_dir, "relative.yaml", "test: content");
        let result = parser.get_final_path("relative.yaml").unwrap();
        let expected = test_dir.join("relative.yaml").canonicalize().unwrap();
        assert_eq!(result, expected);

        // Non-existent path
        let result = parser.get_final_path("nonexistent.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn load_array() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, Some(OsStr::new("yaml")));

        // Mixed content
        create_test_file(&test_dir, "test.yaml", "file_key: file_value");
        let mut direct_hash = Hash::new();
        direct_hash.insert(
            Yaml::String("hash_key".to_string()),
            Yaml::String("hash_value".to_string()),
        );
        let entries = vec![
            Yaml::String("test.yaml".to_string()),
            Yaml::Hash(direct_hash),
        ];
        let result = parser.load_array(&entries, &|_, _| Ok(()));
        assert!(result.is_ok());

        // Invalid entry
        let entries = vec![Yaml::Integer(42)];
        let result = parser.load_array(&entries, &|_, _| Ok(()));
        assert!(result.is_err());
    }

    #[test]
    fn load_path() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, Some(OsStr::new("yaml")));

        // File loading
        create_test_file(&test_dir, "test.yaml", "test_key: test_value");
        let result = parser.load_path("test.yaml", &|_, _| Ok(()));
        assert!(result.is_ok());

        // Directory loading
        let sub_dir = test_dir.join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        create_test_file(&sub_dir, "file1.yaml", "key1: value1");
        create_test_file(&sub_dir, "file2.yaml", "key2: value2");
        create_test_file(&sub_dir, "file3.txt", "ignored"); // Wrong extension
        let result = parser.load_path("subdir", &|_, _| Ok(()));
        assert!(result.is_ok());

        // Non-existent path
        let result = parser.load_path("nonexistent", &|_, _| Ok(()));
        assert!(result.is_err());
    }

    #[test]
    fn load_dir() {
        let test_dir = create_test_dir();

        // With extension filter
        let parser = HybridParser::new(&test_dir, Some(OsStr::new("yml")));
        create_test_file(&test_dir, "config1.yml", "key1: value1");
        create_test_file(&test_dir, "config2.yaml", "key2: value2");
        create_test_file(&test_dir, "config3.txt", "key3: value3");
        let result = parser.load_dir(&test_dir, &|_, _| Ok(()));
        assert!(result.is_ok());

        // Without extension filter
        let parser = HybridParser::new(&test_dir, None);
        let result = parser.load_dir(&test_dir, &|_, _| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn load_file() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, None);

        // Valid YAML with multiple documents
        let test_content = "---\nkey1: value1\n---\nkey2: value2\n";
        let file_path = create_test_file(&test_dir, "multi_doc.yaml", test_content);
        let result = parser.load_file(&file_path, &|_, position| {
            assert!(position.is_some());
            Ok(())
        });
        assert!(result.is_ok());

        // Invalid YAML structure
        let test_content = "- item1\n- item2\n";
        let file_path = create_test_file(&test_dir, "invalid.yaml", test_content);
        let result = parser.load_file(&file_path, &|_, _| Ok(()));
        assert!(result.is_err());
    }

    #[test]
    fn error_handling() {
        let test_dir = create_test_dir();
        let parser = HybridParser::new(&test_dir, None);

        // Path validation error
        create_test_file(&test_dir, "test.yaml", "key: value");
        let yaml_path = Yaml::String("test.yaml".to_string());
        let result = parser.foreach_map(&yaml_path, |_, _| Err(anyhow!("callback error")));
        assert!(result.is_err());

        // String path error context
        let yaml_path = Yaml::String("nonexistent.yaml".to_string());
        let result = parser.foreach_map(&yaml_path, |_, _| Ok(()));
        assert!(result.is_err());

        // Array error context
        let yaml_array = Yaml::Array(vec![Yaml::String("nonexistent.yaml".to_string())]);
        let result = parser.foreach_map(&yaml_array, |_, _| Ok(()));
        assert!(result.is_err());
    }
}
