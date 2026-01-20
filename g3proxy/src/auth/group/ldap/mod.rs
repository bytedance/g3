/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use arc_swap::ArcSwapOption;
use arcstr::ArcStr;

use g3_types::auth::{Password, UserAuthError};
use g3_types::metrics::{MetricTagMap, NodeName};

use super::BaseUserGroup;
use crate::auth::{User, UserContext, UserType};
use crate::config::auth::{LdapUserGroupConfig, UserGroupConfig};

mod protocol;
use protocol::{LdapMessageReceiver, SimpleBindRequestEncoder};

mod pool;
use pool::{LdapAuthPool, LdapAuthPoolHandle};

pub(crate) struct LdapUserGroup {
    base: BaseUserGroup<LdapUserGroupConfig>,
    pool_handle: LdapAuthPoolHandle,
    unmanaged_users: Mutex<AHashMap<ArcStr, Arc<User>>>,
}

impl LdapUserGroup {
    pub(super) fn base(&self) -> &BaseUserGroup<LdapUserGroupConfig> {
        &self.base
    }

    pub(super) fn clone_config(&self) -> LdapUserGroupConfig {
        (*self.base.config).clone()
    }

    fn new(base: BaseUserGroup<LdapUserGroupConfig>) -> anyhow::Result<Self> {
        let pool_handle = LdapAuthPool::create(base.config.clone())?;
        Ok(LdapUserGroup {
            base,
            pool_handle,
            unmanaged_users: Default::default(),
        })
    }

    pub(super) async fn new_with_config(config: LdapUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = BaseUserGroup::new_with_config(config).await?;
        Self::new(base).map(Arc::new)
    }

    pub(super) fn reload(&self, config: LdapUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = self.base.reload(config)?;
        Self::new(base).map(Arc::new)
    }

    pub(super) async fn check_user_with_password(
        &self,
        username: &str,
        password: &Password,
        server_name: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Result<UserContext, UserAuthError> {
        match &self.base.config.unmanaged_user {
            Some(unmanaged_user_config) => {
                self.pool_handle
                    .check_username_password(username, password.as_original())
                    .await?;

                if let Some((user, user_type)) = self.base.get_user(username) {
                    return Ok(UserContext::new(
                        Some(username.into()),
                        user,
                        user_type,
                        server_name,
                        server_extra_tags,
                    ));
                }

                let mut ht = self.unmanaged_users.lock().unwrap();
                match ht.entry(username.into()) {
                    Entry::Occupied(o) => {
                        let user = o.get().clone();
                        Ok(UserContext::new(
                            Some(username.into()),
                            user.clone(),
                            UserType::Unmanaged,
                            server_name,
                            server_extra_tags,
                        ))
                    }
                    Entry::Vacant(v) => {
                        let username = ArcStr::from(username);

                        let user = User::new_unmanaged(
                            &username,
                            self.base.config.basic_config().name(),
                            unmanaged_user_config,
                        )
                        .map_err(|_| UserAuthError::NoSuchUser)?;
                        let user = Arc::new(user);

                        v.insert(user.clone());

                        Ok(UserContext::new(
                            Some(username),
                            user.clone(),
                            UserType::Unmanaged,
                            server_name,
                            server_extra_tags,
                        ))
                    }
                }
            }
            None => {
                if let Some((user, user_type)) = self.base.get_user(username) {
                    self.pool_handle
                        .check_username_password(username, password.as_original())
                        .await?;
                    Ok(UserContext::new(
                        Some(username.into()),
                        user,
                        user_type,
                        server_name,
                        server_extra_tags,
                    ))
                } else {
                    Err(UserAuthError::NoSuchUser)
                }
            }
        }
    }
}
