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
use futures_util::future::{AbortHandle, Abortable};
use log::warn;

use g3_types::metrics::MetricsName;

use super::{User, UserGroupConfig};
use crate::config::auth::{UserConfig, UserDynamicSource};

#[cfg(feature = "lua")]
mod lua;

#[cfg(feature = "python")]
mod python;

pub(super) async fn load_initial_users(
    group: &MetricsName,
    source: &UserDynamicSource,
) -> anyhow::Result<AHashMap<String, Arc<User>>> {
    let r = match source {
        UserDynamicSource::File(config) => config.fetch_records().await?,
        #[cfg(feature = "lua")]
        UserDynamicSource::Lua(config) => config.fetch_cached_records().await?,
        #[cfg(feature = "python")]
        UserDynamicSource::Python(config) => config.fetch_cached_records().await?,
    };

    let datetime_now = Utc::now();
    let mut dynamic_users = AHashMap::new();
    for user_config in r {
        let user_config = Arc::new(user_config);
        let username = user_config.name();
        let user = User::new(group, &user_config, &datetime_now);
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
                        UserDynamicSource::Lua(config) => lua::fetch_records(config).await,
                        #[cfg(feature = "python")]
                        UserDynamicSource::Python(config) => python::fetch_records(config).await,
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
                    // always update
                    Some(Vec::new())
                };

            let datetime_now = Utc::now();

            let old_dynamic_users = dynamic_users_container.load();
            if let Some(dynamic_config) = new_dynamic_config {
                // if fetch success, update/insert/remove dynamic users
                let mut new_dynamic_users = AHashMap::new();
                for user_config in dynamic_config {
                    let user_config = Arc::new(user_config);
                    let username = user_config.name();
                    let user = if let Some(old_user) = old_dynamic_users.get(username) {
                        old_user.new_for_reload(&user_config, &datetime_now)
                    } else {
                        User::new(group_config.name(), &user_config, &datetime_now)
                    };
                    new_dynamic_users.insert(username.to_string(), Arc::new(user));
                }

                dynamic_users_container.store(Arc::new(new_dynamic_users));
            } else {
                // if fetch fail, check expired for old dynamic users
                for (_, user) in old_dynamic_users.iter() {
                    user.check_expired(&datetime_now);
                }
            }

            // check expired for static users
            for (_, user) in static_users.iter() {
                user.check_expired(&datetime_now);
            }

            interval.tick().await;
        }
    };

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let future = Abortable::new(f, abort_registration);
    tokio::spawn(future);
    abort_handle
}
