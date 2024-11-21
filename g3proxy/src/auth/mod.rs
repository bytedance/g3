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

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::anyhow;
use arc_swap::ArcSwap;
use chrono::Utc;
use log::{info, warn};
use tokio::sync::{mpsc, oneshot};

use g3_types::metrics::MetricsName;

use crate::config::auth::UserGroupConfig;

mod ops;
pub use ops::load_all;
pub(crate) use ops::reload;

mod registry;
pub(crate) use registry::{get_all_groups, get_names, get_or_insert_default};

mod site;
pub(crate) use site::UserSite;
use site::UserSites;

mod user;
pub(crate) use user::{User, UserContext};

mod stats;
pub(crate) use stats::{
    UserForbiddenSnapshot, UserForbiddenStats, UserRequestSnapshot, UserRequestStats,
    UserSiteDurationRecorder, UserSiteDurationStats, UserSiteStats, UserTrafficSnapshot,
    UserTrafficStats, UserUpstreamTrafficSnapshot, UserUpstreamTrafficStats,
};

mod source;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UserType {
    Static,
    Dynamic,
    Anonymous,
}

impl UserType {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            UserType::Static => "Static",
            UserType::Dynamic => "Dynamic",
            UserType::Anonymous => "Anonymous",
        }
    }

    fn is_anonymous(&self) -> bool {
        matches!(self, UserType::Anonymous)
    }
}

pub(crate) struct UserGroup {
    config: Arc<UserGroupConfig>,
    static_users: Arc<AHashMap<Arc<str>, Arc<User>>>,
    dynamic_users: Arc<ArcSwap<AHashMap<Arc<str>, Arc<User>>>>,
    /// the job for dynamic fetch
    fetch_quit_sender: Option<mpsc::Sender<()>>,
    // the job for user expire check
    check_quit_sender: Option<oneshot::Sender<()>>,
    anonymous_user: Option<Arc<User>>,
}

impl Drop for UserGroup {
    fn drop(&mut self) {
        if let Some(sender) = self.check_quit_sender.take() {
            let _ = sender.send(());
        }
    }
}

impl UserGroup {
    fn new_without_users(config: UserGroupConfig) -> Self {
        UserGroup {
            config: Arc::new(config),
            static_users: Arc::new(AHashMap::new()),
            dynamic_users: Arc::new(ArcSwap::from_pointee(AHashMap::new())),
            fetch_quit_sender: None,
            check_quit_sender: None,
            anonymous_user: None,
        }
    }

    fn new_no_config(name: &MetricsName) -> Arc<Self> {
        let config = UserGroupConfig::empty(name);
        Arc::new(Self::new_without_users(config))
    }

    async fn new_with_config(config: UserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let datetime_now = Utc::now();
        let mut users = AHashMap::new();
        for (username, user_config) in &config.static_users {
            let user = User::new(config.name(), user_config, &datetime_now)?;
            users.insert(username.clone(), Arc::new(user));
        }

        let anonymous_user = match &config.anonymous_user {
            Some(user_config) => {
                let user = User::new(config.name(), user_config, &datetime_now)?;
                Some(Arc::new(user))
            }
            None => None,
        };

        let mut group = Self::new_without_users(config);
        group.static_users = Arc::new(users);
        if let Some(source) = &group.config.dynamic_source {
            match source::load_initial_users(&group.config, source).await {
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

        group.anonymous_user = anonymous_user;

        group.fetch_quit_sender = Some(source::new_fetch_job(
            group.config.clone(),
            group.dynamic_users.clone(),
        ));
        group.check_quit_sender = Some(source::new_check_job(
            group.config.refresh_interval,
            group.static_users.clone(),
            group.dynamic_users.clone(),
        ));

        Ok(Arc::new(group))
    }

    fn reload(&self, config: UserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let datetime_now = Utc::now();
        let mut static_users = AHashMap::new();
        for (username, user_config) in &config.static_users {
            let user = if let Some(user) = self.static_users.get(username) {
                user.new_for_reload(user_config, &datetime_now)?
            } else {
                User::new(config.name(), user_config, &datetime_now)?
            };
            static_users.insert(username.clone(), Arc::new(user));
        }

        let anonymous_user = match &config.anonymous_user {
            Some(user_config) => {
                let user = if let Some(old) = &self.anonymous_user {
                    old.new_for_reload(user_config, &datetime_now)?
                } else {
                    User::new(config.name(), user_config, &datetime_now)?
                };
                Some(Arc::new(user))
            }
            None => None,
        };

        let mut dynamic_users = AHashMap::new();
        if self.config.dynamic_source.is_some() && config.dynamic_source.is_some() {
            // keep old dynamic users, even if the source may change
            let users = self.dynamic_users.load();
            for (username, user) in users.iter() {
                dynamic_users.insert(username.clone(), Arc::clone(user));
            }
        }

        let mut group = Self::new_without_users(config);
        group.static_users = Arc::new(static_users);
        if !dynamic_users.is_empty() {
            group.dynamic_users.store(Arc::new(dynamic_users));
        }

        group.anonymous_user = anonymous_user;

        group.fetch_quit_sender = Some(source::new_fetch_job(
            group.config.clone(),
            group.dynamic_users.clone(),
        ));
        group.check_quit_sender = Some(source::new_check_job(
            group.config.refresh_interval,
            group.static_users.clone(),
            group.dynamic_users.clone(),
        ));

        Ok(Arc::new(group))
    }

    #[inline]
    pub(crate) fn allow_anonymous(&self, client_addr: SocketAddr) -> bool {
        let Some(user) = &self.anonymous_user else {
            return false;
        };
        user.check_anonymous_client_addr(client_addr).is_ok()
    }

    pub(crate) fn get_anonymous_user(&self) -> Option<(Arc<User>, UserType)> {
        self.anonymous_user
            .as_ref()
            .map(|user| (user.clone(), UserType::Anonymous))
    }

    pub(crate) fn get_user(&self, username: &str) -> Option<(Arc<User>, UserType)> {
        if let Some(user) = self.static_users.get(username) {
            return Some((Arc::clone(user), UserType::Static));
        }

        let dynamic_users = self.dynamic_users.load();
        if let Some(user) = dynamic_users.get(username) {
            return Some((Arc::clone(user), UserType::Dynamic));
        }

        self.get_anonymous_user()
    }

    fn stop_fetch_job(&self) {
        if let Some(sender) = &self.fetch_quit_sender {
            let _ = sender.try_send(());
        }
    }

    pub(crate) fn foreach_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        self.foreach_static_user(&mut f);
        self.foreach_dynamic_user(&mut f);
    }

    pub(crate) fn foreach_static_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        for (name, user) in self.static_users.iter() {
            f(name, user);
        }
    }

    pub(crate) fn foreach_dynamic_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        let dynamic_users = self.dynamic_users.load();
        for (name, user) in dynamic_users.iter() {
            f(name, user);
        }
    }

    pub(crate) fn all_static_users(&self) -> Vec<&str> {
        self.static_users.keys().map(|k| k.as_ref()).collect()
    }

    pub(crate) fn all_dynamic_users(&self) -> Vec<String> {
        let dynamic_users = self.dynamic_users.load();
        dynamic_users.keys().map(|k| k.to_string()).collect()
    }

    pub(crate) async fn publish_dynamic_users(&self, contents: &str) -> anyhow::Result<()> {
        let doc = serde_json::Value::from_str(contents)
            .map_err(|e| anyhow!("the published contents is not valid json: {e}",))?;
        let user_config = crate::config::auth::source::cache::parse_json(&doc)?;

        // we should avoid corrupt write at process exit
        if !self.config.dynamic_cache.as_os_str().is_empty() {
            if let Some(Err(e)) = crate::control::run_protected_io(tokio::fs::write(
                &self.config.dynamic_cache,
                contents,
            ))
            .await
            {
                warn!("failed to cache dynamic users to file {} ({e:?}), this may lead to auth error during restart",
                    self.config.dynamic_cache.display());
            }
        }

        source::publish_dynamic_users(self.config.as_ref(), user_config, &self.dynamic_users)
    }
}
