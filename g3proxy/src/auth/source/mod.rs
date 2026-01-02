/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use arc_swap::ArcSwap;
use arcstr::ArcStr;
use chrono::{DateTime, Utc};
use log::warn;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::User;
use crate::config::auth::{BasicUserGroupConfig, UserDynamicSource, UserGroupConfig};

#[cfg(feature = "lua")]
mod lua;

#[cfg(feature = "python")]
mod python;

pub(super) async fn load_initial_users(
    group_config: &BasicUserGroupConfig,
    source: &UserDynamicSource,
) -> anyhow::Result<AHashMap<ArcStr, Arc<User>>> {
    let (_, all_config) = match source {
        UserDynamicSource::File(config) => config.fetch_records().await?,
        #[cfg(feature = "lua")]
        UserDynamicSource::Lua(config) => {
            config
                .cache(&group_config.dynamic_cache)
                .fetch_records()
                .await?
        }
        #[cfg(feature = "python")]
        UserDynamicSource::Python(config) => {
            config
                .cache(&group_config.dynamic_cache)
                .fetch_records()
                .await?
        }
    };

    let datetime_now = Utc::now();
    let mut dynamic_users = AHashMap::new();
    for user_config in all_config {
        let user_config = Arc::new(user_config);
        let username = user_config.name().clone();
        let user = User::new(group_config.name(), &user_config, &datetime_now)?;
        dynamic_users.insert(username, Arc::new(user));
    }

    Ok(dynamic_users)
}

pub(super) fn new_fetch_job<T>(group_config: Arc<T>, dynamic_key: Uuid) -> mpsc::Sender<()>
where
    T: UserGroupConfig + Send + Sync + 'static,
{
    use mpsc::error::TryRecvError;

    let (quit_sender, mut quit_receiver) = mpsc::channel(1);

    tokio::spawn(async move {
        let basic_config = group_config.basic_config();
        let mut interval = tokio::time::interval(basic_config.refresh_interval);
        interval.tick().await; // will tick immediately
        loop {
            match quit_receiver.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }

            let Some(source) = &basic_config.dynamic_source else {
                break;
            };

            let r = match source {
                UserDynamicSource::File(config) => config.fetch_records().await,
                #[cfg(feature = "lua")]
                UserDynamicSource::Lua(config) => lua::fetch_records(config).await,
                #[cfg(feature = "python")]
                UserDynamicSource::Python(config) => python::fetch_records(config).await,
            };
            match r {
                Ok((contents, dynamic_config)) => {
                    if let Some(group) = super::registry::get(basic_config.name())
                        && let Err(e) = group
                            .save_dynamic_users(&contents, dynamic_config, dynamic_key)
                            .await
                    {
                        warn!(
                            "failed to save dynamic user for group {}: {e:?}",
                            basic_config.name(),
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "failed to fetch dynamic user for group {}: {e:?}",
                        basic_config.name(),
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
    static_users: Arc<AHashMap<ArcStr, Arc<User>>>,
    dynamic_users_container: Arc<ArcSwap<AHashMap<ArcStr, Arc<User>>>>,
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

fn check_dynamic_users(
    datetime_now: &DateTime<Utc>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<ArcStr, Arc<User>>>>,
) {
    let old_dynamic_users = dynamic_users_container.load();
    for (_, user) in old_dynamic_users.iter() {
        user.check_expired(datetime_now);
    }
}

fn check_static_users(
    datetime_now: &DateTime<Utc>,
    static_users: &Arc<AHashMap<ArcStr, Arc<User>>>,
) {
    for (_, user) in static_users.iter() {
        user.check_expired(datetime_now);
    }
}
