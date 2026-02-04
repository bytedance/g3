/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::time::Duration;

use foldhash::fast::FixedState;
use lru::LruCache;
use tokio::time::Instant;

use g3_types::metrics::NodeName;

thread_local! {
    static CACHE: RefCell<HashMap<NodeName, GroupLocalCache, FixedState>> = const {
        RefCell::new(HashMap::with_hasher(FixedState::with_seed(0)))
    };
}

#[derive(Default)]
struct UserLocalCache {
    password_map: BTreeMap<String, Instant>,
}

struct GroupLocalCache {
    user_map: LruCache<String, UserLocalCache, FixedState>,
}

impl Default for GroupLocalCache {
    fn default() -> Self {
        GroupLocalCache::new(crate::config::auth::group::DEFAULT_CACHE_USER_COUNT)
    }
}

impl GroupLocalCache {
    fn new(user_count: NonZeroUsize) -> Self {
        GroupLocalCache {
            user_map: LruCache::with_hasher(user_count, FixedState::with_seed(0)),
        }
    }
}

pub(super) fn has_valid_password(group: &NodeName, username: &str, password: &str) -> bool {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let group = cache
            .entry(group.clone())
            .or_insert_with(GroupLocalCache::default);

        let Some(user) = group.user_map.get_mut(username) else {
            return false;
        };

        let Some((p, t)) = user.password_map.remove_entry(password) else {
            return false;
        };

        if t > Instant::now() {
            user.password_map.insert(p, t);
            true
        } else {
            false
        }
    })
}

pub(super) fn save_user_password(
    group: &NodeName,
    user_count: NonZeroUsize,
    username: String,
    password: String,
    expire_time: Duration,
) {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let group = cache
            .entry(group.clone())
            .and_modify(|g| g.user_map.resize(user_count))
            .or_insert_with(|| GroupLocalCache::new(user_count));

        let user = group
            .user_map
            .get_or_insert_mut(username, UserLocalCache::default);

        user.password_map
            .insert(password, Instant::now() + expire_time);
    })
}
