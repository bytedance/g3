/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_yaml::{HybridParser, YamlDocPosition};

const CONFIG_KEY_USER_GROUP_TYPE: &str = "type";
const CONFIG_KEY_USER_GROUP_NAME: &str = "name";

mod user;
pub(crate) use user::{UserAuditConfig, UserConfig, UserSiteConfig, UsernameParamsConfig};

mod source;
pub(crate) use source::*;

pub(crate) mod group;
pub(crate) use group::{
    AnyUserGroupConfig, BasicUserGroupConfig, FactsUserGroupConfig, UserGroupConfig,
};

mod registry;
pub(crate) use registry::{clear, get_all};

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let group = load_user_group(map, position)?;
        registry::add(group, false)?;
        Ok(())
    })
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyUserGroupConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let group = load_user_group(&map, Some(position.clone()))?;
        registry::add(group.clone(), true)?;
        Ok(group)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_user_group(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyUserGroupConfig> {
    let group_type =
        g3_yaml::hash_get_optional_str(map, CONFIG_KEY_USER_GROUP_TYPE)?.unwrap_or("basic");
    match g3_yaml::key::normalize(group_type).as_str() {
        "basic" => {
            let group = BasicUserGroupConfig::parse(map, position)?;
            Ok(AnyUserGroupConfig::Basic(group))
        }
        "facts" => {
            let group = FactsUserGroupConfig::parse(map, position)?;
            Ok(AnyUserGroupConfig::Facts(group))
        }
        _ => Err(anyhow!("unsupported user group type {group_type}")),
    }
}
