/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use log::warn;
use tokio::sync::{mpsc, oneshot};

use super::{User, UserGroupConfig};
use crate::config::auth::{UserConfig, UserDynamicSource};

#[cfg(feature = "lua")]
mod lua;

#[cfg(all(feature = "python", not(test)))]
mod python;

pub(super) async fn load_initial_users(
    group_config: &UserGroupConfig,
    source: &UserDynamicSource,
) -> anyhow::Result<AHashMap<Arc<str>, Arc<User>>> {
    let r = match source {
        UserDynamicSource::File(config) => config.fetch_records().await?,
        #[cfg(feature = "lua")]
        UserDynamicSource::Lua(config) => {
            config
                .fetch_cached_records(&group_config.dynamic_cache)
                .await?
        }
        #[cfg(all(feature = "python", not(test)))]
        UserDynamicSource::Python(config) => {
            config
                .fetch_cached_records(&group_config.dynamic_cache)
                .await?
        }
    };

    let datetime_now = Utc::now();
    let mut dynamic_users = AHashMap::new();
    for user_config in r {
        let user_config = Arc::new(user_config);
        let username = user_config.name().clone();
        let user = User::new(group_config.name(), &user_config, &datetime_now)?;
        dynamic_users.insert(username, Arc::new(user));
    }

    Ok(dynamic_users)
}

pub(super) fn new_fetch_job(
    group_config: Arc<UserGroupConfig>,
    dynamic_users_container: Arc<ArcSwap<AHashMap<Arc<str>, Arc<User>>>>,
) -> mpsc::Sender<()> {
    use mpsc::error::TryRecvError;

    let (quit_sender, mut quit_receiver) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(group_config.refresh_interval);
        interval.tick().await; // will tick immediately
        loop {
            match quit_receiver.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }

            let Some(source) = &group_config.dynamic_source else {
                break;
            };

            let r = match source {
                UserDynamicSource::File(config) => config.fetch_records().await,
                #[cfg(feature = "lua")]
                UserDynamicSource::Lua(config) => {
                    lua::fetch_records(config, &group_config.dynamic_cache).await
                }
                #[cfg(all(feature = "python", not(test)))]
                UserDynamicSource::Python(config) => {
                    python::fetch_records(config, &group_config.dynamic_cache).await
                }
            };
            match r {
                Ok(dynamic_config) => {
                    if let Err(e) = publish_dynamic_users(
                        group_config.as_ref(),
                        dynamic_config,
                        &dynamic_users_container,
                    ) {
                        warn!("failed to update dynamic users: {e:?}");
                    }
                }
                Err(e) => {
                    warn!(
                        "failed to fetch dynamic user for group {}: {e:?}",
                        group_config.name(),
                    );
                }
            }

            interval.tick().await;
        }
    });

    quit_sender
}

pub(super) fn new_check_job(
    check_interval: Duration,
    static_users: Arc<AHashMap<Arc<str>, Arc<User>>>,
    dynamic_users_container: Arc<ArcSwap<AHashMap<Arc<str>, Arc<User>>>>,
) -> oneshot::Sender<()> {
    use oneshot::error::TryRecvError;

    let (quit_sender, mut quit_receiver) = oneshot::channel();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(check_interval);
        interval.tick().await; // will tick immediately
        loop {
            match quit_receiver.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => break,
            }

            let datetime_now = Utc::now();
            check_dynamic_users(&datetime_now, &dynamic_users_container);
            check_static_users(&datetime_now, &static_users);

            interval.tick().await;
        }
    });

    quit_sender
}

pub(super) fn publish_dynamic_users(
    group_config: &UserGroupConfig,
    dynamic_config: Vec<UserConfig>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<Arc<str>, Arc<User>>>>,
) -> anyhow::Result<()> {
    let datetime_now = Utc::now();
    let old_dynamic_users = dynamic_users_container.load();
    let mut new_dynamic_users = AHashMap::new();
    for user_config in dynamic_config {
        let user_config = Arc::new(user_config);
        let username = user_config.name();
        let user = if let Some(old_user) = old_dynamic_users.get(username.as_ref()) {
            old_user.new_for_reload(&user_config, &datetime_now)?
        } else {
            User::new(group_config.name(), &user_config, &datetime_now)?
        };
        new_dynamic_users.insert(username.clone(), Arc::new(user));
    }

    dynamic_users_container.store(Arc::new(new_dynamic_users));
    Ok(())
}

fn check_dynamic_users(
    datetime_now: &DateTime<Utc>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<Arc<str>, Arc<User>>>>,
) {
    let old_dynamic_users = dynamic_users_container.load();
    for (_, user) in old_dynamic_users.iter() {
        user.check_expired(datetime_now);
    }
}

fn check_static_users(
    datetime_now: &DateTime<Utc>,
    static_users: &Arc<AHashMap<Arc<str>, Arc<User>>>,
) {
    for (_, user) in static_users.iter() {
        user.check_expired(datetime_now);
    }
}
