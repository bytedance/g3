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

use std::sync::Arc;

use ahash::AHashMap;
use arc_swap::ArcSwap;
use chrono::Utc;
use futures_util::future::AbortHandle;
use log::{info, warn};

use crate::config::auth::UserGroupConfig;

mod ops;
pub use ops::load_all;
pub(crate) use ops::reload;

mod registry;
pub(crate) use registry::{get_all_groups, get_names, get_or_insert_default};

mod site;
use site::{UserSite, UserSites};

mod user;
pub(crate) use user::{User, UserContext};

mod stats;
pub(crate) use stats::{
    UserForbiddenSnapshot, UserForbiddenStats, UserRequestSnapshot, UserRequestStats,
    UserSiteStats, UserTrafficSnapshot, UserTrafficStats, UserUpstreamTrafficSnapshot,
    UserUpstreamTrafficStats,
};

mod source;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UserType {
    Static,
    Dynamic,
}

impl UserType {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            UserType::Static => "Static",
            UserType::Dynamic => "Dynamic",
        }
    }
}

pub(crate) struct UserGroup {
    config: Arc<UserGroupConfig>,
    // use ahash for performance
    static_users: Arc<AHashMap<String, Arc<User>>>,
    dynamic_users: Arc<ArcSwap<AHashMap<String, Arc<User>>>>,
    /// the dynamic job is for both dynamic fetch and expire check
    dynamic_job_handler: Option<AbortHandle>,
}

impl Drop for UserGroup {
    fn drop(&mut self) {
        if let Some(handler) = self.dynamic_job_handler.take() {
            handler.abort();
        }
    }
}

impl UserGroup {
    pub(crate) fn name(&self) -> &str {
        self.config.name()
    }

    fn new_without_users(config: UserGroupConfig) -> Self {
        UserGroup {
            config: Arc::new(config),
            static_users: Arc::new(AHashMap::new()),
            dynamic_users: Arc::new(ArcSwap::from_pointee(AHashMap::new())),
            dynamic_job_handler: None,
        }
    }

    fn new_no_config(name: &str) -> Arc<Self> {
        let config = UserGroupConfig::empty(name);
        Arc::new(Self::new_without_users(config))
    }

    async fn new_with_config(config: UserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let datetime_now = Utc::now();
        let mut users = AHashMap::new();
        for (username, user_config) in &config.static_users {
            let user = User::new(config.name(), user_config, &datetime_now);
            users.insert(username.to_string(), Arc::new(user));
        }

        let mut group = Self::new_without_users(config);
        group.static_users = Arc::new(users);
        if let Some(source) = &group.config.dynamic_source {
            match source::load_initial_users(group.config.name(), source).await {
                Ok(cached_users) => {
                    if cached_users.is_empty() {
                        info!(
                            "no cached users found in user-group {}",
                            group.config.name()
                        );
                    } else {
                        group.dynamic_users.store(Arc::new(cached_users));
                    }
                }
                Err(e) => warn!(
                    "failed to load cached dynamic users for user-group {}: {e:?}",
                    group.config.name(),
                ),
            }
        }

        group.dynamic_job_handler = Some(source::new_job(
            &group.config,
            &group.static_users,
            &group.dynamic_users,
        ));

        Ok(Arc::new(group))
    }

    fn reload(&self, config: UserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let datetime_now = Utc::now();
        let mut static_users = AHashMap::new();
        for (username, user_config) in &config.static_users {
            let user = if let Some(user) = self.static_users.get(username) {
                user.new_for_reload(user_config, &datetime_now)
            } else {
                User::new(config.name(), user_config, &datetime_now)
            };
            static_users.insert(username.to_string(), Arc::new(user));
        }

        let mut dynamic_users = AHashMap::new();
        if self.config.dynamic_source.is_some() && config.dynamic_source.is_some() {
            // keep old dynamic users, even if the source may change
            let users = self.dynamic_users.load();
            for (username, user) in users.iter() {
                dynamic_users.insert(username.to_string(), Arc::clone(user));
            }
        }

        let mut group = Self::new_without_users(config);
        group.static_users = Arc::new(static_users);
        if !dynamic_users.is_empty() {
            group.dynamic_users.store(Arc::new(dynamic_users));
        }

        group.dynamic_job_handler = Some(source::new_job(
            &group.config,
            &group.static_users,
            &group.dynamic_users,
        ));

        Ok(Arc::new(group))
    }

    pub(crate) fn get_user(&self, username: &str) -> Option<(Arc<User>, UserType)> {
        if let Some(user) = self.static_users.get(username) {
            return Some((Arc::clone(user), UserType::Static));
        }

        if self.config.dynamic_source.is_some() {
            let dynamic_users = self.dynamic_users.load();
            if let Some(user) = dynamic_users.get(username) {
                return Some((Arc::clone(user), UserType::Dynamic));
            }
        }

        None
    }

    pub(crate) fn foreach_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        self.foreach_static_user(&mut f);
        self.foreach_dynamic_user(&mut f);
    }

    pub(crate) fn foreach_static_user<F>(&self, f: &mut F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        for (name, user) in self.static_users.iter() {
            f(name, user);
        }
    }

    pub(crate) fn foreach_dynamic_user<F>(&self, f: &mut F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        let dynamic_users = self.dynamic_users.load();
        for (name, user) in dynamic_users.iter() {
            f(name, user);
        }
    }

    pub(crate) fn all_static_users(&self) -> Vec<&str> {
        self.static_users.keys().map(|k| k.as_str()).collect()
    }

    pub(crate) fn all_dynamic_users(&self) -> Vec<String> {
        let dynamic_users = self.dynamic_users.load();
        dynamic_users.keys().map(|k| k.to_string()).collect()
    }
}
