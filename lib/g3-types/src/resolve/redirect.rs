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

use std::net::IpAddr;

use ahash::AHashMap;
use radix_trie::Trie;

use super::{reverse_idna_domain, reverse_to_idna_domain, QueryStrategy};
use crate::net::Host;

#[derive(Clone, Eq, PartialEq)]
pub enum ResolveRedirectionValue {
    Domain(String),
    Ip((Vec<IpAddr>, Vec<IpAddr>)),
}

#[derive(Default, Clone, Eq, PartialEq)]
pub struct ResolveRedirectionBuilder {
    ht: AHashMap<String, ResolveRedirectionValue>,
    trie: AHashMap<String, String>,
}

impl ResolveRedirectionBuilder {
    pub fn insert_exact(&mut self, domain: String, ips: Vec<IpAddr>) {
        let mut ipv4 = Vec::new();
        let mut ipv6 = Vec::new();
        for ip in ips {
            match ip {
                IpAddr::V4(_) => ipv4.push(ip),
                IpAddr::V6(ip6) => {
                    if let Some(ip4) = ip6.to_ipv4_mapped() {
                        ipv4.push(IpAddr::V4(ip4));
                    } else {
                        ipv6.push(ip);
                    }
                }
            }
        }

        self.ht
            .insert(domain, ResolveRedirectionValue::Ip((ipv4, ipv6)));
    }

    pub fn insert_exact_alias(&mut self, domain: String, alias: String) {
        self.ht
            .insert(domain, ResolveRedirectionValue::Domain(alias));
    }

    pub fn insert_parent(&mut self, from: String, to: String) {
        self.trie.insert(from, to);
    }

    pub fn build(&self) -> ResolveRedirection {
        let mut trie = Trie::new();
        for (k, v) in self.trie.iter() {
            // append extra '.' to match the exact parent domain
            let from = reverse_idna_domain(k);
            let to = reverse_idna_domain(v);
            let node = TrieValue {
                from: from.clone(),
                to,
            };
            trie.insert(from, node);
        }

        ResolveRedirection {
            ht: self.ht.clone(),
            match_trie: !self.trie.is_empty(),
            trie,
        }
    }
}

struct TrieValue {
    from: String,
    to: String,
}

pub struct ResolveRedirection {
    ht: AHashMap<String, ResolveRedirectionValue>,
    match_trie: bool,
    trie: Trie<String, TrieValue>,
}

impl ResolveRedirection {
    pub fn query_value(&self, domain: &str) -> Option<ResolveRedirectionValue> {
        if !self.ht.is_empty() {
            if let Some(v) = self.ht.get(domain) {
                return Some(v.clone());
            }
        }

        if self.match_trie {
            let reversed_domain = reverse_idna_domain(domain);
            if let Some(node) = self.trie.get_ancestor_value(&reversed_domain) {
                let replaced = reversed_domain.replacen(&node.from, &node.to, 1);
                return Some(ResolveRedirectionValue::Domain(reverse_to_idna_domain(
                    &replaced,
                )));
            }
        }

        None
    }

    pub fn query_first(&self, domain: &str, strategy: QueryStrategy) -> Option<Host> {
        if !self.ht.is_empty() {
            if let Some(v) = self.ht.get(domain) {
                match v {
                    ResolveRedirectionValue::Domain(alias) => {
                        return Some(Host::Domain(alias.to_string()));
                    }
                    ResolveRedirectionValue::Ip((ip4, ip6)) => match strategy {
                        QueryStrategy::Ipv4Only => {
                            if !ip4.is_empty() {
                                return Some(Host::Ip(ip4[0]));
                            }
                        }
                        QueryStrategy::Ipv6Only => {
                            if !ip6.is_empty() {
                                return Some(Host::Ip(ip6[0]));
                            }
                        }
                        QueryStrategy::Ipv4First => {
                            if !ip4.is_empty() {
                                return Some(Host::Ip(ip4[0]));
                            }
                            if !ip6.is_empty() {
                                return Some(Host::Ip(ip6[0]));
                            }
                        }
                        QueryStrategy::Ipv6First => {
                            if !ip6.is_empty() {
                                return Some(Host::Ip(ip6[0]));
                            }
                            if !ip4.is_empty() {
                                return Some(Host::Ip(ip4[0]));
                            }
                        }
                    },
                }
            }
        }

        if self.match_trie {
            let reversed_domain = reverse_idna_domain(domain);
            if let Some(node) = self.trie.get_ancestor_value(&reversed_domain) {
                let replaced = reversed_domain.replacen(&node.from, &node.to, 1);
                return Some(Host::Domain(reverse_to_idna_domain(&replaced)));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::str::FromStr;

    const DOMAIN1: &str = "www.example1.com";
    const DOMAIN2: &str = "www.example2.com";
    const DOMAIN3: &str = "www.example3.com";
    const DOMAIN4: &str = "www.example4.com";

    #[test]
    fn exact_replace_ips() {
        let mut builder = ResolveRedirectionBuilder::default();
        let ip41 = IpAddr::from_str("1.1.1.1").unwrap();
        let ip42 = IpAddr::from_str("2.2.2.2").unwrap();
        let ip61 = IpAddr::from_str("2001:20::1").unwrap();
        let ip62 = IpAddr::from_str("2001:21::1").unwrap();
        let target_ips1 = vec![ip41, ip42];
        let target_ips2 = vec![ip61, ip62];
        let target_ips3 = vec![ip41, ip42, ip61, ip62];

        builder.insert_exact(DOMAIN1.to_string(), target_ips1);
        builder.insert_exact(DOMAIN2.to_string(), target_ips2);
        builder.insert_exact(DOMAIN3.to_string(), target_ips3);
        let r = builder.build();

        let ret = r.query_first(DOMAIN1, QueryStrategy::Ipv4Only).unwrap();
        assert_eq!(ret, Host::Ip(ip41));
        let ret = r.query_first(DOMAIN1, QueryStrategy::Ipv4First).unwrap();
        assert_eq!(ret, Host::Ip(ip41));
        let ret = r.query_first(DOMAIN1, QueryStrategy::Ipv6First).unwrap();
        assert_eq!(ret, Host::Ip(ip41));
        assert!(r.query_first(DOMAIN1, QueryStrategy::Ipv6Only).is_none());

        let ret = r.query_first(DOMAIN2, QueryStrategy::Ipv6Only).unwrap();
        assert_eq!(ret, Host::Ip(ip61));
        let ret = r.query_first(DOMAIN2, QueryStrategy::Ipv6First).unwrap();
        assert_eq!(ret, Host::Ip(ip61));
        let ret = r.query_first(DOMAIN2, QueryStrategy::Ipv4First).unwrap();
        assert_eq!(ret, Host::Ip(ip61));
        assert!(r.query_first(DOMAIN2, QueryStrategy::Ipv4Only).is_none());

        let ret = r.query_first(DOMAIN3, QueryStrategy::Ipv4Only).unwrap();
        assert_eq!(ret, Host::Ip(ip41));
        let ret = r.query_first(DOMAIN3, QueryStrategy::Ipv4First).unwrap();
        assert_eq!(ret, Host::Ip(ip41));
        let ret = r.query_first(DOMAIN3, QueryStrategy::Ipv6Only).unwrap();
        assert_eq!(ret, Host::Ip(ip61));
        let ret = r.query_first(DOMAIN3, QueryStrategy::Ipv6First).unwrap();
        assert_eq!(ret, Host::Ip(ip61));

        assert!(r.query_first(DOMAIN4, QueryStrategy::Ipv4First).is_none());
    }

    #[test]
    fn exact_replace_alias() {
        let mut builder = ResolveRedirectionBuilder::default();
        let to_domain = "www.1-example.com";
        builder.insert_exact_alias(DOMAIN1.to_string(), to_domain.to_string());
        let r = builder.build();

        let ret = r.query_first(DOMAIN1, QueryStrategy::Ipv4First).unwrap();
        assert_eq!(ret, Host::Domain(to_domain.to_string()));

        assert!(r.query_first(DOMAIN4, QueryStrategy::Ipv4First).is_none());
    }

    #[test]
    fn parent_replace() {
        let mut builder = ResolveRedirectionBuilder::default();
        builder.insert_parent("foo.com".to_string(), "bar.com".to_string());
        let r = builder.build();

        let ret = r.query_first("foo.com", QueryStrategy::Ipv4First).unwrap();
        assert_eq!(ret, Host::Domain("bar.com".to_string()));
        let ret = r
            .query_first("a.foo.com", QueryStrategy::Ipv4First)
            .unwrap();
        assert_eq!(ret, Host::Domain("a.bar.com".to_string()));

        assert!(r
            .query_first("a.zfoo.com", QueryStrategy::Ipv4First)
            .is_none());
        assert!(r
            .query_first("a.fooz.com", QueryStrategy::Ipv4First)
            .is_none());
    }
}
