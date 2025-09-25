/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::fs::ConfigFileFormat;

pub fn as_file_path(v: &Yaml, lookup_dir: &Path, auto_create: bool) -> anyhow::Result<PathBuf> {
    if let Yaml::String(path) = v {
        let path = PathBuf::from_str(path).map_err(|e| anyhow!("invalid path: {e:?}"))?;
        let path = if path.is_absolute() {
            path
        } else {
            let mut abs_path = lookup_dir.to_path_buf();
            abs_path.push(path);
            abs_path
        };
        if path.exists() {
            if !path.is_file() {
                return Err(anyhow!("the path is existed but not a regular file"));
            }
        } else if auto_create {
            if let Some(dir_path) = path.parent() {
                std::fs::create_dir_all(dir_path).map_err(|e| {
                    anyhow!("failed to create parent dir {}: {e:?}", dir_path.display())
                })?;
                let _ = File::create(&path)
                    .map_err(|e| anyhow!("failed to create file {}: {e:?}", path.display()))?;
            } else {
                return Err(anyhow!("the path has no valid parent dir"));
            }
        } else {
            return Err(anyhow!("path {} is not existed", path.display()));
        }
        path.canonicalize()
            .map_err(|e| anyhow!("invalid path {}: {e:?}", path.display()))
    } else {
        Err(anyhow!("yaml value type for path should be string"))
    }
}

pub fn as_file(v: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<(File, PathBuf)> {
    let path = if let Some(dir) = lookup_dir {
        as_file_path(v, dir, false)?
    } else {
        as_absolute_path(v)?
    };
    let file =
        File::open(&path).map_err(|e| anyhow!("failed to open file({}): {e:?}", path.display()))?;
    Ok((file, path))
}

pub fn as_absolute_path(v: &Yaml) -> anyhow::Result<PathBuf> {
    if let Yaml::String(path) = v {
        let path = PathBuf::from_str(path).map_err(|e| anyhow!("invalid path: {e:?}"))?;
        if path.is_relative() {
            return Err(anyhow!(
                "invalid value: {} is not an absolute path",
                path.display()
            ));
        }
        Ok(path)
    } else {
        Err(anyhow!(
            "yaml value type for absolute path should be string"
        ))
    }
}

pub fn as_config_file_format(v: &Yaml) -> anyhow::Result<ConfigFileFormat> {
    if let Yaml::String(s) = v {
        Ok(ConfigFileFormat::from_str(s)
            .map_err(|_| anyhow!("invalid config file format string"))?)
    } else {
        Err(anyhow!(
            "yaml value type for config file format should be string"
        ))
    }
}

pub fn as_dir_path(v: &Yaml, lookup_dir: &Path, auto_create: bool) -> anyhow::Result<PathBuf> {
    if let Yaml::String(path) = v {
        let path = PathBuf::from_str(path).map_err(|e| anyhow!("invalid path: {e:?}"))?;
        let path = if path.is_absolute() {
            path
        } else {
            let mut abs_path = lookup_dir.to_path_buf();
            abs_path.push(path);
            abs_path
        };
        if path.exists() {
            if !path.is_dir() {
                return Err(anyhow!("the path is existed but not a directory"));
            }
        } else if auto_create {
            std::fs::create_dir_all(&path)
                .map_err(|e| anyhow!("failed to create dir {}: {e:?}", path.display()))?;
        } else {
            return Err(anyhow!("path {} is not existed", path.display()));
        }
        path.canonicalize()
            .map_err(|e| anyhow!("invalid path {}: {e:?}", path.display()))
    } else {
        Err(anyhow!("yaml value type for dir path should be string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_DIR_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let id = TEST_DIR_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), id));
            fs::create_dir_all(&path).expect("Failed to create test directory");
            TempDir { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn as_file_path_ok() {
        let temp_dir = TempDir::new("as_file_path_ok");
        let file_path = temp_dir.path().join("test.txt");
        fs::File::create(&file_path).unwrap();

        // existing absolute path
        let yaml = Yaml::String(file_path.to_str().unwrap().to_string());
        assert_eq!(
            as_file_path(&yaml, temp_dir.path(), false).unwrap(),
            file_path.canonicalize().unwrap()
        );

        // existing relative path with lookup_dir
        let yaml = yaml_str!("test.txt");
        assert_eq!(
            as_file_path(&yaml, temp_dir.path(), false).unwrap(),
            file_path.canonicalize().unwrap()
        );

        // non-existing path with auto_create true
        let new_file_path = temp_dir.path().join("new.txt");
        let yaml = Yaml::String(new_file_path.to_str().unwrap().to_string());
        assert_eq!(
            as_file_path(&yaml, temp_dir.path(), true).unwrap(),
            new_file_path.canonicalize().unwrap()
        );
        assert!(new_file_path.exists());
    }

    #[test]
    fn as_file_path_err() {
        let temp_dir = TempDir::new("as_file_path_err");

        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_file_path(&yaml, temp_dir.path(), false).is_err());

        // existing path that is not a file
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        let yaml = Yaml::String(sub_dir.to_str().unwrap().to_string());
        assert!(as_file_path(&yaml, temp_dir.path(), false).is_err());

        // non-existing path with auto_create false
        let non_existing_path = temp_dir.path().join("non_existing.txt");
        let yaml = Yaml::String(non_existing_path.to_str().unwrap().to_string());
        assert!(as_file_path(&yaml, temp_dir.path(), false).is_err());

        // invalid path creation
        let invalid_path = temp_dir.path().join("invalid\u{e000}dir\u{0000}test.txt");
        let yaml = Yaml::String(invalid_path.to_str().unwrap().to_string());
        assert!(as_file_path(&yaml, temp_dir.path(), true).is_err());
    }

    #[test]
    fn as_file_ok() {
        let temp_dir = TempDir::new("as_file_ok");
        let file_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"test").unwrap();

        // absolute path without lookup_dir
        let yaml = Yaml::String(file_path.to_str().unwrap().to_string());
        let (opened_file, path) = as_file(&yaml, None).unwrap();
        assert_eq!(
            path.canonicalize().unwrap(),
            file_path.canonicalize().unwrap()
        );
        drop(opened_file);

        // relative path with lookup_dir
        let relative_yaml = yaml_str!("test.txt");
        let (opened_file, path) = as_file(&relative_yaml, Some(temp_dir.path())).unwrap();
        assert_eq!(
            path.canonicalize().unwrap(),
            file_path.canonicalize().unwrap()
        );
        drop(opened_file);
    }

    #[test]
    fn as_file_err() {
        let temp_dir = TempDir::new("as_file_err");

        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_file(&yaml, None).is_err());

        // non-existing absolute path
        let non_existing_path = temp_dir.path().join("non_existing.txt");
        let yaml = Yaml::String(non_existing_path.to_str().unwrap().to_string());
        assert!(as_file(&yaml, None).is_err());
    }

    #[test]
    fn as_absolute_path_ok() {
        let temp_dir = TempDir::new("as_absolute_path_ok");
        let absolute_path = temp_dir.path().to_str().unwrap();

        // valid absolute path
        let yaml = Yaml::String(absolute_path.to_string());
        assert_eq!(
            as_absolute_path(&yaml).unwrap(),
            PathBuf::from(absolute_path)
        );
    }

    #[test]
    fn as_absolute_path_err() {
        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_absolute_path(&yaml).is_err());

        // relative path
        let yaml = yaml_str!("relative/path");
        assert!(as_absolute_path(&yaml).is_err());
    }

    #[test]
    fn as_config_file_format_ok() {
        // valid formats
        let yaml = yaml_str!("yaml");
        assert!(matches!(
            as_config_file_format(&yaml).unwrap(),
            ConfigFileFormat::Yaml
        ));

        let yaml = yaml_str!("yml");
        assert!(matches!(
            as_config_file_format(&yaml).unwrap(),
            ConfigFileFormat::Yaml
        ));

        let yaml = yaml_str!("json");
        assert!(matches!(
            as_config_file_format(&yaml).unwrap(),
            ConfigFileFormat::Json
        ));

        // case insensitivity
        let yaml = yaml_str!("YAML");
        assert!(matches!(
            as_config_file_format(&yaml).unwrap(),
            ConfigFileFormat::Yaml
        ));
    }

    #[test]
    fn as_config_file_format_err() {
        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_config_file_format(&yaml).is_err());

        // invalid format string
        let yaml = yaml_str!("invalid");
        assert!(as_config_file_format(&yaml).is_err());
    }

    #[test]
    fn as_dir_path_ok() {
        let temp_dir = TempDir::new("as_dir_path_ok");
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // existing absolute path
        let yaml = Yaml::String(sub_dir.to_str().unwrap().to_string());
        assert_eq!(
            as_dir_path(&yaml, temp_dir.path(), false).unwrap(),
            sub_dir.canonicalize().unwrap()
        );

        // existing relative path with lookup_dir
        let yaml = yaml_str!("subdir");
        assert_eq!(
            as_dir_path(&yaml, temp_dir.path(), false).unwrap(),
            sub_dir.canonicalize().unwrap()
        );

        // non-existing path with auto_create true
        let new_dir_path = temp_dir.path().join("newdir");
        let yaml = Yaml::String(new_dir_path.to_str().unwrap().to_string());
        assert_eq!(
            as_dir_path(&yaml, temp_dir.path(), true).unwrap(),
            new_dir_path.canonicalize().unwrap()
        );
        assert!(new_dir_path.exists());
    }

    #[test]
    fn as_dir_path_err() {
        let temp_dir = TempDir::new("as_dir_path_err");

        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_dir_path(&yaml, temp_dir.path(), false).is_err());

        // existing path that is not a directory
        let file_path = temp_dir.path().join("test.txt");
        fs::File::create(&file_path).unwrap();
        let yaml = Yaml::String(file_path.to_str().unwrap().to_string());
        assert!(as_dir_path(&yaml, temp_dir.path(), false).is_err());

        // non-existing path with auto_create false
        let non_existing_path = temp_dir.path().join("non_existing_dir");
        let yaml = Yaml::String(non_existing_path.to_str().unwrap().to_string());
        assert!(as_dir_path(&yaml, temp_dir.path(), false).is_err());

        // invalid path creation
        let invalid_path = temp_dir.path().join("invalid\u{e000}dir\u{0000}test.txt");
        let yaml = Yaml::String(invalid_path.to_str().unwrap().to_string());
        assert!(as_dir_path(&yaml, temp_dir.path(), true).is_err());
    }
}
