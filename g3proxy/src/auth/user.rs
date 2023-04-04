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

use std::cmp::PartialEq;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use arc_swap::ArcSwapOption;
use chrono::{DateTime, Utc};
use governor::{clock::DefaultClock, state::InMemoryState, state::NotKeyed, RateLimiter};
use http::HeaderMap;
use tokio::time::Instant;

use g3_types::acl::AclAction;
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::auth::UserAuthError;
use g3_types::limit::{GaugeSemaphore, GaugeSemaphorePermit};
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{ProxyRequestType, UpstreamAddr};
use g3_types::resolve::{ResolveRedirection, ResolveStrategy};

use super::{
    UserForbiddenStats, UserRequestStats, UserSite, UserSiteStats, UserSites, UserTrafficStats,
    UserType, UserUpstreamTrafficStats,
};
use crate::config::auth::UserConfig;

pub(crate) struct User {
    pub(crate) config: Arc<UserConfig>,
    group: MetricsName,
    started: Instant,
    is_expired: AtomicBool,
    is_blocked: Arc<AtomicBool>,
    request_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    tcp_conn_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    resolve_redirection: Option<ResolveRedirection>,
    log_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    forbid_stats: Arc<Mutex<AHashMap<String, Arc<UserForbiddenStats>>>>,
    req_stats: Arc<Mutex<AHashMap<String, Arc<UserRequestStats>>>>,
    io_stats: Arc<Mutex<AHashMap<String, Arc<UserTrafficStats>>>>,
    upstream_io_stats: Arc<Mutex<AHashMap<String, Arc<UserUpstreamTrafficStats>>>>,
    req_alive_sem: GaugeSemaphore,
    explicit_sites: UserSites,
}

impl User {
    #[inline]
    pub(crate) fn name(&self) -> &str {
        self.config.name()
    }

    #[inline]
    pub(crate) fn task_max_idle_count(&self) -> i32 {
        self.config.task_idle_max_count
    }

    fn update_dst_host_filter(&mut self) {
        self.dst_host_filter = self
            .config
            .dst_host_filter
            .as_ref()
            .map(|builder| Arc::new(builder.build()));
    }

    fn update_resolve_redirection(&mut self) {
        self.resolve_redirection = self
            .config
            .resolve_redirection
            .as_ref()
            .map(|builder| builder.build());
    }

    pub(super) fn new(
        group: &MetricsName,
        config: &Arc<UserConfig>,
        datetime_now: &DateTime<Utc>,
    ) -> Self {
        let request_rate_limit = config
            .request_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let tcp_conn_rate_limit = config
            .tcp_conn_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let log_rate_limit = config
            .log_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));

        let is_expired = AtomicBool::new(config.is_expired(datetime_now));
        let is_blocked = Arc::new(AtomicBool::new(config.block_and_delay.is_some()));

        let explicit_sites = UserSites::new(config.explicit_sites.values(), config.name(), group);

        let mut user = User {
            config: Arc::clone(config),
            group: group.clone(),
            started: Instant::now(),
            is_expired,
            is_blocked,
            request_rate_limit,
            tcp_conn_rate_limit,
            dst_host_filter: None,
            resolve_redirection: None,
            log_rate_limit,
            forbid_stats: Arc::new(Mutex::new(AHashMap::new())),
            req_stats: Arc::new(Mutex::new(AHashMap::new())),
            io_stats: Arc::new(Mutex::new(AHashMap::new())),
            upstream_io_stats: Arc::new(Mutex::new(AHashMap::new())),
            req_alive_sem: GaugeSemaphore::new(config.request_alive_max),
            explicit_sites,
        };
        user.update_dst_host_filter();
        user.update_resolve_redirection();
        user
    }

    pub(super) fn new_for_reload(
        &self,
        config: &Arc<UserConfig>,
        datetime_now: &DateTime<Utc>,
    ) -> Self {
        let request_rate_limit = if let Some(quota) = &config.request_rate_limit {
            if let Some(old_limiter) = &self.request_rate_limit {
                if let Some(old_quota) = &self.config.request_rate_limit {
                    if quota.eq(old_quota) {
                        // always use the old rate limiter when possible
                        Some(Arc::clone(old_limiter))
                    } else {
                        Some(Arc::new(RateLimiter::direct(quota.get_inner())))
                    }
                } else {
                    unreachable!()
                }
            } else {
                Some(Arc::new(RateLimiter::direct(quota.get_inner())))
            }
        } else {
            None
        };

        let tcp_conn_rate_limit = if let Some(quota) = &config.tcp_conn_rate_limit {
            if let Some(old_limiter) = &self.tcp_conn_rate_limit {
                if let Some(old_quota) = &self.config.tcp_conn_rate_limit {
                    if quota.eq(old_quota) {
                        // always use the old rate limiter when possible
                        Some(Arc::clone(old_limiter))
                    } else {
                        Some(Arc::new(RateLimiter::direct(quota.get_inner())))
                    }
                } else {
                    unreachable!()
                }
            } else {
                Some(Arc::new(RateLimiter::direct(quota.get_inner())))
            }
        } else {
            None
        };

        let log_rate_limit = if let Some(quota) = &config.log_rate_limit {
            if let Some(old_limiter) = &self.log_rate_limit {
                if let Some(old_quota) = &self.config.log_rate_limit {
                    if quota.eq(old_quota) {
                        // always use the old rate limiter when possible
                        Some(Arc::clone(old_limiter))
                    } else {
                        Some(Arc::new(RateLimiter::direct(quota.get_inner())))
                    }
                } else {
                    unreachable!()
                }
            } else {
                Some(Arc::new(RateLimiter::direct(quota.get_inner())))
            }
        } else {
            None
        };

        // always use the expired state in new config
        let is_expired = AtomicBool::new(config.is_expired(datetime_now));

        // use the latest block state in new config
        if config.block_and_delay.is_some() {
            self.is_blocked.fetch_or(true, Ordering::Relaxed);
        } else {
            self.is_blocked.fetch_and(false, Ordering::Relaxed);
        }
        let is_blocked = Arc::clone(&self.is_blocked);

        let explicit_sites = self.explicit_sites.new_for_reload(
            config.explicit_sites.values(),
            config.name(),
            &self.group,
        );

        let mut user = User {
            config: Arc::clone(config),
            group: self.group.clone(),
            started: self.started,
            is_expired,
            is_blocked,
            request_rate_limit,
            tcp_conn_rate_limit,
            dst_host_filter: None,
            resolve_redirection: None,
            log_rate_limit,
            forbid_stats: Arc::clone(&self.forbid_stats),
            req_stats: Arc::clone(&self.req_stats),
            io_stats: Arc::clone(&self.io_stats),
            upstream_io_stats: Arc::clone(&self.upstream_io_stats),
            req_alive_sem: self.req_alive_sem.new_updated(config.request_alive_max),
            explicit_sites,
        };
        if self.config.dst_host_filter.ne(&config.dst_host_filter) {
            user.update_dst_host_filter();
        } else {
            user.dst_host_filter = self.dst_host_filter.clone();
        }
        user.update_resolve_redirection();
        user
    }

    /// for user blocked check in idle checking
    pub(crate) fn is_blocked(&self) -> bool {
        self.is_blocked.load(Ordering::Relaxed)
    }

    #[inline]
    fn is_expired(&self) -> bool {
        self.is_expired.load(Ordering::Relaxed)
    }

    pub(super) fn check_expired(&self, datetime_now: &DateTime<Utc>) -> bool {
        if self.config.is_expired(datetime_now) {
            // TODO log user expire ?
            self.is_expired.swap(true, Ordering::Relaxed);
            true
        } else {
            // it's not possible for expired users to be valid with out new config reload
            false
        }
    }

    fn check_password(
        &self,
        password: &str,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> Result<(), UserAuthError> {
        if !self.config.check_password(password) {
            forbid_stats.add_auth_failed();
            return Err(UserAuthError::TokenNotMatch);
        }
        if self.is_expired() {
            forbid_stats.add_user_expired();
            return Err(UserAuthError::ExpiredUser);
        }
        if let Some(duration) = self.config.block_and_delay {
            forbid_stats.add_user_blocked();
            return Err(UserAuthError::BlockedUser(duration));
        }
        Ok(())
    }

    fn fetch_forbidden_stats(
        &self,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserForbiddenStats> {
        let mut map = self.forbid_stats.lock().unwrap();
        let stats = map.entry(server.to_string()).or_insert_with(|| {
            Arc::new(UserForbiddenStats::new(
                &self.group,
                self.name(),
                user_type,
                server,
                server_extra_tags,
            ))
        });
        Arc::clone(stats)
    }

    pub(crate) fn all_forbidden_stats(&self) -> Vec<Arc<UserForbiddenStats>> {
        let map = self.forbid_stats.lock().unwrap();
        let mut all_stats = Vec::with_capacity(map.len());
        for stats in map.values() {
            all_stats.push(Arc::clone(stats));
        }
        all_stats
    }

    fn fetch_request_stats(
        &self,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserRequestStats> {
        let mut map = self.req_stats.lock().unwrap();
        let stats = map.entry(server.to_string()).or_insert_with(|| {
            Arc::new(UserRequestStats::new(
                &self.group,
                self.name(),
                user_type,
                server,
                server_extra_tags,
            ))
        });
        Arc::clone(stats)
    }

    pub(crate) fn all_request_stats(&self) -> Vec<Arc<UserRequestStats>> {
        let map = self.req_stats.lock().unwrap();
        let mut all_stats = Vec::with_capacity(map.len());
        for stats in map.values() {
            all_stats.push(Arc::clone(stats));
        }
        all_stats
    }

    fn fetch_traffic_stats(
        &self,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserTrafficStats> {
        let mut map = self.io_stats.lock().unwrap();
        let stats = map.entry(server.to_string()).or_insert_with(|| {
            Arc::new(UserTrafficStats::new(
                &self.group,
                self.name(),
                user_type,
                server,
                server_extra_tags,
            ))
        });
        Arc::clone(stats)
    }

    pub(crate) fn all_traffic_stats(&self) -> Vec<Arc<UserTrafficStats>> {
        let map = self.io_stats.lock().unwrap();
        let mut all_stats = Vec::with_capacity(map.len());
        for stats in map.values() {
            all_stats.push(Arc::clone(stats));
        }
        all_stats
    }

    fn fetch_upstream_traffic_stats(
        &self,
        user_type: UserType,
        escaper: &MetricsName,
        escaper_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Arc<UserUpstreamTrafficStats> {
        let mut map = self.upstream_io_stats.lock().unwrap();
        let stats = map.entry(escaper.to_string()).or_insert_with(|| {
            Arc::new(UserUpstreamTrafficStats::new(
                &self.group,
                self.name(),
                user_type,
                escaper,
                escaper_extra_tags,
            ))
        });
        Arc::clone(stats)
    }

    pub(crate) fn all_upstream_traffic_stats(&self) -> Vec<Arc<UserUpstreamTrafficStats>> {
        let map = self.upstream_io_stats.lock().unwrap();
        let mut all_stats = Vec::with_capacity(map.len());
        for stats in map.values() {
            all_stats.push(Arc::clone(stats));
        }
        all_stats
    }

    fn skip_log(&self, forbid_stats: &Arc<UserForbiddenStats>) -> bool {
        if let Some(limit) = &self.log_rate_limit {
            if limit.check().is_err() {
                forbid_stats.add_log_skipped();
                return true;
            }
        }
        false
    }

    fn check_rate_limit(
        &self,
        reused_connection: bool,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> Result<(), ()> {
        if !reused_connection {
            if let Some(limit) = &self.tcp_conn_rate_limit {
                if limit.check().is_err() {
                    forbid_stats.add_rate_limited();
                    return Err(());
                }
            }
        }
        if let Some(limit) = &self.request_rate_limit {
            if limit.check().is_err() {
                forbid_stats.add_rate_limited();
                return Err(());
            }
        }
        Ok(())
    }

    fn acquire_request_semaphore(
        &self,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> Result<GaugeSemaphorePermit, ()> {
        self.req_alive_sem.try_acquire().map_err(|_| {
            forbid_stats.add_fully_loaded();
        })
    }

    fn check_proxy_request(
        &self,
        request: ProxyRequestType,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> AclAction {
        if let Some(filter) = &self.config.proxy_request_filter {
            let (_, action) = filter.check_request(&request);
            if action.forbid_early() {
                forbid_stats.add_proto_banned();
            }
            action
        } else {
            AclAction::Permit
        }
    }

    fn check_upstream(
        &self,
        upstream: &UpstreamAddr,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> AclAction {
        let mut default_action = AclAction::Permit;

        if let Some(filter) = &self.config.dst_port_filter {
            let port = upstream.port();
            let (found, action) = filter.check_port(&port);
            if found && action.forbid_early() {
                forbid_stats.add_dest_denied();
                return action;
            };
            default_action = default_action.restrict(action);
        }

        if let Some(filter) = &self.dst_host_filter {
            let (found, action) = filter.check(upstream.host());
            if found && action.forbid_early() {
                forbid_stats.add_dest_denied();
                return action;
            }
            default_action = default_action.restrict(action);
        }

        if default_action.forbid_early() {
            forbid_stats.add_dest_denied();
        }
        default_action
    }

    fn check_http_user_agent(
        &self,
        headers: &HeaderMap,
        forbid_stats: &Arc<UserForbiddenStats>,
    ) -> Option<AclAction> {
        if let Some(filter) = &self.config.http_user_agent_filter {
            let mut default_action = filter.missed_action();
            for v in headers.get_all(http::header::USER_AGENT) {
                let ua_value = match v.to_str() {
                    Ok(s) => s,
                    Err(_) => {
                        forbid_stats.add_ua_blocked();
                        return Some(AclAction::Forbid);
                    }
                };
                if let (true, action) = filter.check(ua_value) {
                    if action.forbid_early() {
                        forbid_stats.add_ua_blocked();
                        return Some(action);
                    }
                    default_action = default_action.restrict(action);
                }
            }
            Some(default_action)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn resolve_redirection(&self) -> Option<&ResolveRedirection> {
        self.resolve_redirection.as_ref()
    }
}

#[derive(Clone)]
pub(crate) struct UserContext {
    user: Arc<User>,
    user_type: UserType,
    user_site: Option<Arc<UserSite>>,
    forbid_stats: Arc<UserForbiddenStats>,
    req_stats: Arc<UserRequestStats>,
    site_stats: Option<Arc<UserSiteStats>>,
    site_req_stats: Option<Arc<UserRequestStats>>,
    reused_client_connection: bool,
}

impl UserContext {
    pub(crate) fn new(
        user: Arc<User>,
        user_type: UserType,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Self {
        let forbid_stats = user.fetch_forbidden_stats(user_type, server, server_extra_tags);
        let req_stats = user.fetch_request_stats(user_type, server, server_extra_tags);
        UserContext {
            user,
            user_type,
            user_site: None,
            forbid_stats,
            req_stats,
            site_stats: None,
            site_req_stats: None,
            reused_client_connection: false,
        }
    }

    pub(crate) fn mark_reused_client_connection(&mut self) {
        self.reused_client_connection = true;
    }

    pub(crate) fn check_in_site(
        &mut self,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
        ups: &UpstreamAddr,
    ) {
        if let Some(user_site) = self.user.explicit_sites.fetch_site(ups) {
            if user_site.emit_stats() {
                let user_site_stats = user_site.stats().clone();
                self.site_req_stats = Some(user_site_stats.fetch_request_stats(
                    self.user_type,
                    server,
                    server_extra_tags,
                ));
                self.site_stats = Some(user_site_stats);
            }

            self.user_site = Some(user_site);
        }
    }

    #[inline]
    pub(crate) fn user(&self) -> &Arc<User> {
        &self.user
    }

    pub(crate) fn resolve_strategy(&self) -> Option<ResolveStrategy> {
        self.user_site
            .as_ref()
            .and_then(|s| s.resolve_strategy())
            .or(self.user.config.resolve_strategy)
    }

    #[inline]
    pub(crate) fn forbidden_stats(&self) -> &Arc<UserForbiddenStats> {
        &self.forbid_stats
    }

    #[inline]
    pub(crate) fn req_stats(&self) -> &Arc<UserRequestStats> {
        &self.req_stats
    }

    #[inline]
    pub(crate) fn site_req_stats(&self) -> Option<&Arc<UserRequestStats>> {
        self.site_req_stats.as_ref()
    }

    pub(crate) fn fetch_traffic_stats(
        &self,
        server: &str,
        server_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Vec<Arc<UserTrafficStats>> {
        let mut all_stats = Vec::with_capacity(2);

        all_stats.push(
            self.user
                .fetch_traffic_stats(self.user_type, server, server_extra_tags),
        );

        if let Some(site) = &self.site_stats {
            all_stats.push(site.fetch_traffic_stats(self.user_type, server, server_extra_tags));
        }

        all_stats
    }

    pub(crate) fn fetch_upstream_traffic_stats(
        &self,
        escaper: &MetricsName,
        escaper_extra_tags: &Arc<ArcSwapOption<StaticMetricsTags>>,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        let mut all_stats = Vec::with_capacity(2);

        all_stats.push(self.user.fetch_upstream_traffic_stats(
            self.user_type,
            escaper,
            escaper_extra_tags,
        ));

        if let Some(site) = &self.site_stats {
            all_stats.push(site.fetch_upstream_traffic_stats(
                self.user_type,
                escaper,
                escaper_extra_tags,
            ));
        }

        all_stats
    }

    #[inline]
    pub(crate) fn check_password(&self, password: &str) -> Result<(), UserAuthError> {
        self.user.check_password(password, &self.forbid_stats)
    }

    #[inline]
    pub(crate) fn skip_log(&self) -> bool {
        self.user.skip_log(&self.forbid_stats)
    }

    #[inline]
    pub(crate) fn check_rate_limit(&self) -> Result<(), ()> {
        self.user
            .check_rate_limit(self.reused_client_connection, &self.forbid_stats)
    }

    #[inline]
    pub(crate) fn acquire_request_semaphore(&self) -> Result<GaugeSemaphorePermit, ()> {
        self.user.acquire_request_semaphore(&self.forbid_stats)
    }

    #[inline]
    pub(crate) fn check_proxy_request(&self, request: ProxyRequestType) -> AclAction {
        self.user.check_proxy_request(request, &self.forbid_stats)
    }

    #[inline]
    pub(crate) fn check_upstream(&self, upstream: &UpstreamAddr) -> AclAction {
        self.user.check_upstream(upstream, &self.forbid_stats)
    }

    #[inline]
    pub(crate) fn check_http_user_agent(&self, headers: &HeaderMap) -> Option<AclAction> {
        self.user.check_http_user_agent(headers, &self.forbid_stats)
    }

    #[inline]
    pub(crate) fn add_dest_denied(&self) {
        self.forbid_stats.add_dest_denied();
    }

    #[inline]
    pub(crate) fn add_ip_blocked(&self) {
        self.forbid_stats.add_ip_blocked();
    }
}
