/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::anyhow;
use arc_swap::{ArcSwap, ArcSwapOption};
use arcstr::ArcStr;
use chrono::Utc;
use log::{info, warn};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use g3_types::auth::{Password, UserAuthError};
use g3_types::metrics::{MetricTagMap, NodeName};

use super::{User, UserContext, UserType, source};
use crate::config::auth::{AnyUserGroupConfig, UserConfig, UserGroupConfig};

mod basic;
pub(crate) use basic::BasicUserGroup;

mod facts;
pub(crate) use facts::FactsUserGroup;

#[derive(Clone)]
pub(crate) enum UserGroup {
    Basic(Arc<BasicUserGroup>),
    Facts(Arc<FactsUserGroup>),
}

impl UserGroup {
    pub(super) fn r#type(&self) -> &'static str {
        match self {
            UserGroup::Basic(v) => v.base().r#type(),
            UserGroup::Facts(v) => v.base().r#type(),
        }
    }

    pub(super) fn clone_config(&self) -> AnyUserGroupConfig {
        match self {
            UserGroup::Basic(v) => {
                let c = v.clone_config();
                AnyUserGroupConfig::Basic(c)
            }
            UserGroup::Facts(v) => {
                let c = v.clone_config();
                AnyUserGroupConfig::Facts(c)
            }
        }
    }

    pub(super) fn new_no_config(name: &NodeName) -> Self {
        let group = BasicUserGroup::new_no_config(name);
        UserGroup::Basic(group)
    }

    pub(super) async fn new_with_config(config: AnyUserGroupConfig) -> anyhow::Result<Self> {
        match config {
            AnyUserGroupConfig::Basic(c) => {
                let group = BasicUserGroup::new_with_config(c).await?;
                Ok(UserGroup::Basic(group))
            }
            AnyUserGroupConfig::Facts(c) => {
                let group = FactsUserGroup::new_with_config(c).await?;
                Ok(UserGroup::Facts(group))
            }
        }
    }

    pub(super) fn reload(&self, config: AnyUserGroupConfig) -> anyhow::Result<Self> {
        match (self, config) {
            (UserGroup::Basic(g), AnyUserGroupConfig::Basic(c)) => {
                let group = g.reload(c)?;
                Ok(UserGroup::Basic(group))
            }
            (UserGroup::Facts(g), AnyUserGroupConfig::Facts(c)) => {
                let group = g.reload(c)?;
                Ok(UserGroup::Facts(group))
            }
            (_, config) => Err(anyhow!(
                "reload user group {} type {} to {} is invalid",
                config.name(),
                self.r#type(),
                config.r#type(),
            )),
        }
    }

    pub(super) fn stop_fetch_job(&self) {
        match self {
            UserGroup::Basic(v) => v.base().stop_fetch_job(),
            UserGroup::Facts(v) => v.base().stop_fetch_job(),
        }
    }

    pub(crate) fn allow_anonymous(&self, client_addr: SocketAddr) -> bool {
        match self {
            UserGroup::Basic(v) => v.base().allow_anonymous(client_addr),
            UserGroup::Facts(v) => v.base().allow_anonymous(client_addr),
        }
    }

    pub(crate) fn get_anonymous_user(&self) -> Option<(Arc<User>, UserType)> {
        match self {
            UserGroup::Basic(v) => v.base().get_anonymous_user(),
            UserGroup::Facts(v) => v.base().get_anonymous_user(),
        }
    }

    pub(crate) fn foreach_user<F>(&self, f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        match self {
            UserGroup::Basic(v) => v.base().foreach_user(f),
            UserGroup::Facts(v) => v.base().foreach_user(f),
        }
    }

    pub(crate) fn all_static_users(&self) -> Vec<&str> {
        match self {
            UserGroup::Basic(v) => v.base().all_static_users(),
            UserGroup::Facts(v) => v.base().all_static_users(),
        }
    }

    pub(crate) fn all_dynamic_users(&self) -> Vec<String> {
        match self {
            UserGroup::Basic(v) => v.base().all_dynamic_users(),
            UserGroup::Facts(v) => v.base().all_dynamic_users(),
        }
    }

    pub(crate) async fn publish_dynamic_users(&self, contents: &str) -> anyhow::Result<()> {
        match self {
            UserGroup::Basic(v) => v.base().publish_dynamic_users(contents).await,
            UserGroup::Facts(v) => v.base().publish_dynamic_users(contents).await,
        }
    }

    pub(super) async fn save_dynamic_users(
        &self,
        contents: &str,
        dynamic_config: Vec<UserConfig>,
        dynamic_key: Uuid,
    ) -> anyhow::Result<()> {
        match self {
            UserGroup::Basic(v) => {
                v.base()
                    .save_dynamic_users(contents, dynamic_config, Some(dynamic_key))
                    .await
            }
            UserGroup::Facts(v) => {
                v.base()
                    .save_dynamic_users(contents, dynamic_config, Some(dynamic_key))
                    .await
            }
        }
    }

    pub(crate) fn check_user_with_password(
        &self,
        username: &str,
        password: &Password,
        server_name: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Result<UserContext, UserAuthError> {
        match self {
            UserGroup::Basic(v) => v.base().check_user_with_password(
                username,
                password,
                server_name,
                server_extra_tags,
            ),
            UserGroup::Facts(_) => Err(UserAuthError::NoSuchUser),
        }
    }
}

struct BaseUserGroup<T: UserGroupConfig> {
    config: Arc<T>,
    static_users: Arc<AHashMap<ArcStr, Arc<User>>>,
    dynamic_key: Uuid,
    dynamic_users: Arc<ArcSwap<AHashMap<ArcStr, Arc<User>>>>,
    /// the job for dynamic fetch
    fetch_quit_sender: Option<mpsc::Sender<()>>,
    // the job for user expire check
    check_quit_sender: Option<oneshot::Sender<()>>,
    anonymous_user: Option<Arc<User>>,
}

impl<T: UserGroupConfig> Drop for BaseUserGroup<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.check_quit_sender.take() {
            let _ = sender.send(());
        }
    }
}

impl<T> BaseUserGroup<T>
where
    T: UserGroupConfig + Send + Sync + 'static,
{
    fn r#type(&self) -> &'static str {
        self.config.r#type()
    }

    fn new_without_users(config: T) -> Self {
        BaseUserGroup {
            config: Arc::new(config),
            static_users: Arc::new(AHashMap::new()),
            dynamic_key: Uuid::new_v4(),
            dynamic_users: Arc::new(ArcSwap::from_pointee(AHashMap::new())),
            fetch_quit_sender: None,
            check_quit_sender: None,
            anonymous_user: None,
        }
    }

    async fn new_with_config(config: T) -> anyhow::Result<Self> {
        let basic_config = config.basic_config();

        let datetime_now = Utc::now();
        let mut users = AHashMap::new();
        for (username, user_config) in &basic_config.static_users {
            let user = User::new(basic_config.name(), user_config, &datetime_now)?;
            users.insert(username.clone(), Arc::new(user));
        }

        let anonymous_user = match &basic_config.anonymous_user {
            Some(user_config) => {
                let user = User::new(basic_config.name(), user_config, &datetime_now)?;
                Some(Arc::new(user))
            }
            None => None,
        };

        let mut group = Self::new_without_users(config);
        let basic_config = group.config.basic_config();

        group.static_users = Arc::new(users);
        if let Some(source) = &basic_config.dynamic_source {
            match source::load_initial_users(basic_config, source).await {
                Ok(cached_users) => {
                    if cached_users.is_empty() {
                        info!(
                            "no cached users found in user-group {}",
                            basic_config.name()
                        );
                    } else {
                        group.dynamic_users.store(Arc::new(cached_users));
                    }
                }
                Err(e) => warn!(
                    "failed to load cached dynamic users for user-group {}: {e:?}",
                    basic_config.name(),
                ),
            }
        }

        group.anonymous_user = anonymous_user;

        group.fetch_quit_sender = Some(source::new_fetch_job(
            group.config.clone(),
            group.dynamic_key,
        ));
        group.check_quit_sender = Some(source::new_check_job(
            basic_config.refresh_interval,
            group.static_users.clone(),
            group.dynamic_users.clone(),
        ));

        Ok(group)
    }

    fn reload(&self, config: T) -> anyhow::Result<Self> {
        let basic_config = config.basic_config();

        let datetime_now = Utc::now();
        let mut static_users = AHashMap::new();
        for (username, user_config) in &basic_config.static_users {
            let user = if let Some(user) = self.static_users.get(username) {
                user.new_for_reload(user_config, &datetime_now)?
            } else {
                User::new(basic_config.name(), user_config, &datetime_now)?
            };
            static_users.insert(username.clone(), Arc::new(user));
        }

        let anonymous_user = match &basic_config.anonymous_user {
            Some(user_config) => {
                let user = if let Some(old) = &self.anonymous_user {
                    old.new_for_reload(user_config, &datetime_now)?
                } else {
                    User::new(basic_config.name(), user_config, &datetime_now)?
                };
                Some(Arc::new(user))
            }
            None => None,
        };

        let mut dynamic_users = AHashMap::new();
        if self.config.basic_config().dynamic_source.is_some()
            && basic_config.dynamic_source.is_some()
        {
            // keep old dynamic users, even if the source may change
            let users = self.dynamic_users.load();
            for (username, user) in users.iter() {
                dynamic_users.insert(username.clone(), Arc::clone(user));
            }
        }

        let mut group = Self::new_without_users(config);
        let basic_config = group.config.basic_config();

        group.static_users = Arc::new(static_users);
        if !dynamic_users.is_empty() {
            group.dynamic_users.store(Arc::new(dynamic_users));
        }

        group.anonymous_user = anonymous_user;

        group.fetch_quit_sender = Some(source::new_fetch_job(
            group.config.clone(),
            group.dynamic_key,
        ));
        group.check_quit_sender = Some(source::new_check_job(
            basic_config.refresh_interval,
            group.static_users.clone(),
            group.dynamic_users.clone(),
        ));

        Ok(group)
    }

    async fn publish_dynamic_users(&self, contents: &str) -> anyhow::Result<()> {
        let doc = serde_json::Value::from_str(contents)
            .map_err(|e| anyhow!("the published contents is not valid json: {e}"))?;
        let users = UserConfig::parse_json_many(&doc)?;
        self.save_dynamic_users(contents, users, None).await
    }

    async fn save_dynamic_users(
        &self,
        contents: &str,
        dynamic_config: Vec<UserConfig>,
        dynamic_key: Option<Uuid>,
    ) -> anyhow::Result<()> {
        if let Some(key) = dynamic_key
            && key != self.dynamic_key
        {
            return Ok(());
        }

        let basic_config = self.config.basic_config();
        // we should avoid corrupt write at process exit
        if !basic_config.dynamic_cache.as_os_str().is_empty()
            && let Some(Err(e)) = crate::control::run_protected_io(tokio::fs::write(
                &basic_config.dynamic_cache,
                contents,
            ))
            .await
        {
            warn!(
                "failed to cache dynamic users to file {} ({e:?}), this may lead to auth error during restart",
                basic_config.dynamic_cache.display()
            );
        }

        let datetime_now = Utc::now();
        let old_dynamic_users = self.dynamic_users.load();
        let mut new_dynamic_users = AHashMap::new();
        for user_config in dynamic_config {
            let user_config = Arc::new(user_config);
            let username = user_config.name();
            let user = if let Some(old_user) = old_dynamic_users.get(username) {
                old_user.new_for_reload(&user_config, &datetime_now)?
            } else {
                User::new(basic_config.name(), &user_config, &datetime_now)?
            };
            new_dynamic_users.insert(username.clone(), Arc::new(user));
        }

        self.dynamic_users.store(Arc::new(new_dynamic_users));
        Ok(())
    }

    fn allow_anonymous(&self, client_addr: SocketAddr) -> bool {
        let Some(user) = &self.anonymous_user else {
            return false;
        };
        user.check_anonymous_client_addr(client_addr).is_ok()
    }

    fn get_anonymous_user(&self) -> Option<(Arc<User>, UserType)> {
        self.anonymous_user
            .as_ref()
            .map(|user| (user.clone(), UserType::Anonymous))
    }

    fn get_user(&self, username: &str) -> Option<(Arc<User>, UserType)> {
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

    fn foreach_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        self.foreach_static_user(&mut f);
        self.foreach_dynamic_user(&mut f);
    }

    fn foreach_static_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        for (name, user) in self.static_users.iter() {
            f(name, user);
        }
    }

    fn foreach_dynamic_user<F>(&self, mut f: F)
    where
        F: FnMut(&str, &Arc<User>),
    {
        let dynamic_users = self.dynamic_users.load();
        for (name, user) in dynamic_users.iter() {
            f(name, user);
        }
    }

    fn all_static_users(&self) -> Vec<&str> {
        self.static_users.keys().map(|k| k.as_ref()).collect()
    }

    fn all_dynamic_users(&self) -> Vec<String> {
        let dynamic_users = self.dynamic_users.load();
        dynamic_users.keys().map(|k| k.to_string()).collect()
    }

    fn check_user_with_password(
        &self,
        username: &str,
        password: &Password,
        server_name: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
    ) -> Result<UserContext, UserAuthError> {
        let Some((user, user_type)) = self.get_user(username) else {
            return Err(UserAuthError::NoSuchUser);
        };
        let user_ctx = UserContext::new(
            Some(username.into()),
            user,
            user_type,
            server_name,
            server_extra_tags,
        );
        user_ctx.check_password(password.as_original())?;
        Ok(user_ctx)
    }
}
