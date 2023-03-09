/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YamlDocPosition {
    pub path: PathBuf,
    pub index: usize,
}

impl YamlDocPosition {
    // FIXME use TryFrom trait after no conflicting implementation in 'core'
    pub fn from_str_iter<'a, I: Iterator<Item = &'a str>>(mut iter: I) -> anyhow::Result<Self> {
        let path = match iter.next() {
            Some(path) => path,
            None => return Err(anyhow!("no path found")),
        };

        match iter.next() {
            Some(index) => {
                let index = usize::from_str(index)?;
                Ok(YamlDocPosition {
                    path: PathBuf::from(path),
                    index,
                })
            }
            None => YamlDocPosition::from_str(path),
        }
    }
}

impl FromStr for YamlDocPosition {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let mut iter = s.split('#');
        let path = match iter.next() {
            Some(path) => path.to_string(),
            None => return Err(anyhow!("no path found")),
        };

        match iter.next() {
            Some(index) => {
                let index = usize::from_str(index)?;
                Ok(YamlDocPosition {
                    path: PathBuf::from(path),
                    index,
                })
            }
            None => Ok(YamlDocPosition {
                path: PathBuf::from(path),
                index: 0,
            }),
        }
    }
}

impl fmt::Display for YamlDocPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.path.display(), self.index)
    }
}

pub fn load_doc(position: &YamlDocPosition) -> anyhow::Result<Yaml> {
    let mut conf = String::new();
    File::open(&position.path)?.read_to_string(&mut conf)?;

    let mut yaml_docs = YamlLoader::load_from_str(&conf)?;
    if yaml_docs.get(position.index).is_some() {
        Ok(yaml_docs.remove(position.index))
    } else {
        Err(anyhow!("no doc found in {position}"))
    }
}

pub fn foreach_doc<F>(path: &Path, f: F) -> anyhow::Result<()>
where
    F: Fn(usize, &Yaml) -> anyhow::Result<()>,
{
    let mut conf = String::new();
    File::open(path)?.read_to_string(&mut conf)?;

    let yaml_docs = YamlLoader::load_from_str(&conf)?;
    for (i, doc) in yaml_docs.iter().enumerate() {
        f(i, doc)?;
    }
    Ok(())
}
