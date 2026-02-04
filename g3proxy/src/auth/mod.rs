/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod ops;
pub use ops::load_all;
pub(crate) use ops::reload;

mod registry;
pub(crate) use registry::{get_all_groups, get_names, get_or_insert_default};

mod cache;

mod site;
pub(crate) use site::UserSite;
use site::UserSites;

mod user;
pub(crate) use user::{User, UserContext};

mod group;
pub(crate) use group::{FactsUserGroup, UserGroup};

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
    Unmanaged,
    Anonymous,
}

impl UserType {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            UserType::Static => "Static",
            UserType::Dynamic => "Dynamic",
            UserType::Unmanaged => "Unmanaged",
            UserType::Anonymous => "Anonymous",
        }
    }

    fn is_anonymous(&self) -> bool {
        matches!(self, UserType::Anonymous)
    }
}
