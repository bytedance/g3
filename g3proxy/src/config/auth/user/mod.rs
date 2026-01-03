/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use arcstr::ArcStr;
use chrono::{DateTime, Utc};

use g3_types::acl::{
    AclExactPortRule, AclNetworkRuleBuilder, AclProxyRequestRule, AclUserAgentRule,
};
use g3_types::acl_set::AclDstHostRuleSetBuilder;
use g3_types::auth::FactsMatchValue;
use g3_types::limit::{
    GlobalDatagramSpeedLimitConfig, GlobalStreamSpeedLimitConfig, RateLimitQuota,
};
use g3_types::metrics::NodeName;
use g3_types::net::{
    HttpKeepAliveConfig, TcpConnectConfig, TcpKeepAliveConfig, TcpMiscSockOpts,
    TcpSockSpeedLimitConfig, UdpMiscSockOpts, UdpSockSpeedLimitConfig,
};
use g3_types::resolve::{ResolveRedirectionBuilder, ResolveStrategy};

use crate::escape::EgressPathSelection;

mod token;
use token::PasswordToken;

mod site;
pub(crate) use site::UserSiteConfig;

mod audit;
pub(crate) use audit::UserAuditConfig;

mod name_params;
pub(crate) use name_params::UsernameParamsConfig;

mod json;
mod yaml;

#[derive(Clone)]
pub(crate) struct UserConfig {
    name: ArcStr,
    password_token: PasswordToken,
    pub(crate) match_by_facts: Vec<FactsMatchValue>,
    expire_datetime: Option<DateTime<Utc>>,
    pub(crate) audit: UserAuditConfig,
    pub(crate) block_and_delay: Option<Duration>,
    pub(crate) tcp_connect: Option<TcpConnectConfig>,
    pub(crate) tcp_remote_keepalive: TcpKeepAliveConfig,
    tcp_remote_misc_opts: Option<TcpMiscSockOpts>,
    udp_remote_misc_opts: Option<UdpMiscSockOpts>,
    tcp_client_misc_opts: Option<TcpMiscSockOpts>,
    udp_client_misc_opts: Option<UdpMiscSockOpts>,
    pub(crate) http_upstream_keepalive: HttpKeepAliveConfig,
    pub(crate) http_rsp_hdr_recv_timeout: Option<Duration>,
    pub(crate) request_alive_max: usize,
    pub(crate) request_rate_limit: Option<RateLimitQuota>,
    pub(crate) connection_rate_limit: Option<RateLimitQuota>,
    pub(crate) tcp_sock_speed_limit: TcpSockSpeedLimitConfig,
    pub(crate) udp_sock_speed_limit: UdpSockSpeedLimitConfig,
    pub(crate) tcp_all_upload_speed_limit: Option<GlobalStreamSpeedLimitConfig>,
    pub(crate) tcp_all_download_speed_limit: Option<GlobalStreamSpeedLimitConfig>,
    pub(crate) udp_all_upload_speed_limit: Option<GlobalDatagramSpeedLimitConfig>,
    pub(crate) udp_all_download_speed_limit: Option<GlobalDatagramSpeedLimitConfig>,
    pub(crate) log_rate_limit: Option<RateLimitQuota>,
    pub(crate) log_uri_max_chars: Option<usize>,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) proxy_request_filter: Option<AclProxyRequestRule>,
    pub(crate) dst_host_filter: Option<AclDstHostRuleSetBuilder>,
    pub(crate) dst_port_filter: Option<AclExactPortRule>,
    pub(crate) http_user_agent_filter: Option<AclUserAgentRule>,
    pub(crate) resolve_strategy: Option<ResolveStrategy>,
    pub(crate) resolve_redirection: Option<ResolveRedirectionBuilder>,
    pub(crate) task_idle_max_count: Option<usize>,
    pub(crate) socks_use_udp_associate: bool,
    pub(crate) egress_path_selection: Option<EgressPathSelection>,
    pub(crate) explicit_sites: BTreeMap<NodeName, Arc<UserSiteConfig>>,
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            name: Default::default(),
            password_token: PasswordToken::Forbidden,
            match_by_facts: Vec::new(),
            expire_datetime: None,
            audit: UserAuditConfig::default(),
            block_and_delay: None,
            tcp_connect: None,
            tcp_remote_keepalive: Default::default(),
            tcp_remote_misc_opts: None,
            udp_remote_misc_opts: None,
            tcp_client_misc_opts: None,
            udp_client_misc_opts: None,
            http_upstream_keepalive: Default::default(),
            http_rsp_hdr_recv_timeout: None,
            request_alive_max: 0,
            request_rate_limit: None,
            connection_rate_limit: None,
            tcp_sock_speed_limit: Default::default(),
            udp_sock_speed_limit: Default::default(),
            tcp_all_upload_speed_limit: None,
            tcp_all_download_speed_limit: None,
            udp_all_upload_speed_limit: None,
            udp_all_download_speed_limit: None,
            log_rate_limit: None,
            log_uri_max_chars: None,
            ingress_net_filter: None,
            proxy_request_filter: None,
            dst_host_filter: None,
            dst_port_filter: None,
            http_user_agent_filter: None,
            resolve_strategy: None,
            resolve_redirection: None,
            task_idle_max_count: None,
            socks_use_udp_associate: false,
            egress_path_selection: None,
            explicit_sites: BTreeMap::new(),
        }
    }
}

impl UserConfig {
    pub(crate) fn name(&self) -> &ArcStr {
        &self.name
    }

    pub(crate) fn is_expired(&self, dt_now: &DateTime<Utc>) -> bool {
        if let Some(dt_expire) = &self.expire_datetime {
            dt_expire.lt(dt_now)
        } else {
            false
        }
    }

    pub(crate) fn check_password(&self, password: &str) -> bool {
        match &self.password_token {
            PasswordToken::Forbidden => false,
            PasswordToken::SkipVerify => true,
            PasswordToken::FastHash(fast_hash) => fast_hash.verify(password).unwrap(),
            PasswordToken::XCrypt(xcrypt_hash) => xcrypt_hash.verify(password.as_bytes()).unwrap(),
        }
    }

    pub(super) fn set_no_password(&mut self) {
        self.password_token = PasswordToken::SkipVerify;
    }

    fn add_site_group(&mut self, sg: UserSiteConfig) -> anyhow::Result<()> {
        let name = sg.id.clone();
        if let Some(old_sg) = self.explicit_sites.insert(name, Arc::new(sg)) {
            Err(anyhow!(
                "duplicate config for user site group {}",
                old_sg.id
            ))
        } else {
            Ok(())
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }

        let mut check_exact_ip = BTreeSet::new();
        let mut check_exact_domain = BTreeSet::new();
        let mut check_child_domain = BTreeSet::new();
        let mut check_subnet = BTreeSet::new();
        for config in self.explicit_sites.values() {
            for ip in &config.exact_match_ipaddr {
                if !check_exact_ip.insert(*ip) {
                    return Err(anyhow!(
                        "IP address {ip} in site group {} has already been added by others",
                        config.id
                    ));
                }
            }
            for domain in &config.exact_match_domain {
                if !check_exact_domain.insert(domain) {
                    return Err(anyhow!(
                        "Exact Domain {domain} in site group {} has already been added by others",
                        config.id
                    ));
                }
            }
            for domain in &config.child_match_domain {
                if !check_child_domain.insert(domain.strip_prefix('.').unwrap_or(domain)) {
                    return Err(anyhow!(
                        "Parent Domain {domain} in site group {} has already been added by others",
                        config.id
                    ));
                }
            }
            for net in &config.subnet_match_ipaddr {
                if !check_subnet.insert(*net) {
                    return Err(anyhow!(
                        "Subnet {net} in site group {} has already been added by others",
                        config.id
                    ));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn tcp_remote_misc_opts<'a>(
        &self,
        base_opts: &'a TcpMiscSockOpts,
    ) -> Cow<'a, TcpMiscSockOpts> {
        if let Some(user_opts) = &self.tcp_remote_misc_opts {
            Cow::Owned(user_opts.adjust_to(base_opts))
        } else {
            Cow::Borrowed(base_opts)
        }
    }

    pub(crate) fn udp_remote_misc_opts(&self, base_opts: &UdpMiscSockOpts) -> UdpMiscSockOpts {
        if let Some(user_opts) = self.udp_remote_misc_opts {
            user_opts.adjust_to(base_opts)
        } else {
            *base_opts
        }
    }

    pub(crate) fn tcp_client_misc_opts<'a>(
        &self,
        base_opts: &'a TcpMiscSockOpts,
    ) -> Cow<'a, TcpMiscSockOpts> {
        if let Some(user_opts) = &self.tcp_client_misc_opts {
            Cow::Owned(user_opts.adjust_to(base_opts))
        } else {
            Cow::Borrowed(base_opts)
        }
    }

    pub(crate) fn udp_client_misc_opts(&self, base_opts: &UdpMiscSockOpts) -> UdpMiscSockOpts {
        if let Some(user_opts) = self.udp_client_misc_opts {
            user_opts.adjust_to(base_opts)
        } else {
            *base_opts
        }
    }
}
