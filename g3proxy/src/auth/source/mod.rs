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
use chrono::{DateTime, Utc};
use futures_util::future::{AbortHandle, Abortable};
use log::warn;

use super::{User, UserGroupConfig};
use crate::config::auth::{UserConfig, UserDynamicSource};

#[cfg(feature = "lua")]
mod lua;

#[cfg(feature = "python")]
mod python;

pub(super) async fn load_initial_users(
    group_config: &UserGroupConfig,
    source: &UserDynamicSource,
) -> anyhow::Result<AHashMap<String, Arc<User>>> {
    let r = match source {
        UserDynamicSource::File(config) => config.fetch_records().await?,
        #[cfg(feature = "lua")]
        UserDynamicSource::Lua(config) => {
            config
                .fetch_cached_records(&group_config.dynamic_cache)
                .await?
        }
        #[cfg(feature = "python")]
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
        let username = user_config.name();
        let user = User::new(group_config.name(), &user_config, &datetime_now)?;
        dynamic_users.insert(username.to_string(), Arc::new(user));
    }

    Ok(dynamic_users)
}

pub(super) fn new_job(
    group_config: &Arc<UserGroupConfig>,
    static_users: &Arc<AHashMap<String, Arc<User>>>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<String, Arc<User>>>>,
) -> AbortHandle {
    let group_config = Arc::clone(group_config);
    let static_users = Arc::clone(static_users);
    let dynamic_users_container = Arc::clone(dynamic_users_container);

    let f = async move {
        let mut interval = tokio::time::interval(group_config.refresh_interval);
        interval.tick().await; // will tick immediately
        loop {
            let new_dynamic_config: Option<Vec<UserConfig>> =
                if let Some(source) = &group_config.dynamic_source {
                    let r = match source {
                        UserDynamicSource::File(config) => config.fetch_records().await,
                        #[cfg(feature = "lua")]
                        UserDynamicSource::Lua(config) => {
                            lua::fetch_records(config, &group_config.dynamic_cache).await
                        }
                        #[cfg(feature = "python")]
                        UserDynamicSource::Python(config) => {
                            python::fetch_records(config, &group_config.dynamic_cache).await
                        }
                    };
                    match r {
                        Ok(users) => Some(users),
                        Err(e) => {
                            warn!(
                                "failed to fetch dynamic user for group {}: {e:?}",
                                group_config.name(),
                            );
                            None
                        }
                    }
                } else {
                    None
                };

            let datetime_now = Utc::now();

            if let Some(dynamic_config) = new_dynamic_config {
                // if fetch success, update/insert/remove dynamic users
                if let Err(e) = update_dynamic_users(
                    group_config.as_ref(),
                    &datetime_now,
                    dynamic_config,
                    &dynamic_users_container,
                ) {
                    warn!("failed to update dynamic users: {e:?}");
                }
            } else {
                // if fetch fail or no need to fetch, check expired for old dynamic users
                check_dynamic_users(&datetime_now, &dynamic_users_container);
            }

            check_static_users(&datetime_now, &static_users);

            interval.tick().await;
        }
    };

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let future = Abortable::new(f, abort_registration);
    tokio::spawn(future);
    abort_handle
}

pub(super) fn publish_dynamic_users(
    group_config: &UserGroupConfig,
    dynamic_config: Vec<UserConfig>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<String, Arc<User>>>>,
) -> anyhow::Result<()> {
    let datetime_now = Utc::now();
    update_dynamic_users(
        group_config,
        &datetime_now,
        dynamic_config,
        dynamic_users_container,
    )
}

fn update_dynamic_users(
    group_config: &UserGroupConfig,
    datetime_now: &DateTime<Utc>,
    dynamic_config: Vec<UserConfig>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<String, Arc<User>>>>,
) -> anyhow::Result<()> {
    let old_dynamic_users = dynamic_users_container.load();
    let mut new_dynamic_users = AHashMap::new();
    for user_config in dynamic_config {
        let user_config = Arc::new(user_config);
        let username = user_config.name();
        let user = if let Some(old_user) = old_dynamic_users.get(username) {
            old_user.new_for_reload(&user_config, datetime_now)?
        } else {
            User::new(group_config.name(), &user_config, datetime_now)?
        };
        new_dynamic_users.insert(username.to_string(), Arc::new(user));
    }

    dynamic_users_container.store(Arc::new(new_dynamic_users));
    Ok(())
}

fn check_dynamic_users(
    datetime_now: &DateTime<Utc>,
    dynamic_users_container: &Arc<ArcSwap<AHashMap<String, Arc<User>>>>,
) {
    let old_dynamic_users = dynamic_users_container.load();
    for (_, user) in old_dynamic_users.iter() {
        user.check_expired(datetime_now);
    }
}

fn check_static_users(
    datetime_now: &DateTime<Utc>,
    static_users: &Arc<AHashMap<String, Arc<User>>>,
) {
    for (_, user) in static_users.iter() {
        user.check_expired(datetime_now);
    }
}
