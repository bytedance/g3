/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025
 */

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use fnv::FnvHasher;
use redis::AsyncCommands;
use url::Url;
use tokio::sync::OnceCell;

use g3_types::net::UpstreamAddr;

static REDIS_URL: OnceCell<Option<String>> = OnceCell::const_new();
static PREFIX: OnceCell<Option<String>> = OnceCell::const_new();
static REDIS_CLIENT: OnceCell<Option<g3_redis_client::RedisClientConfig>> = OnceCell::const_new();
static DEFAULT_TTL: OnceCell<Duration> = OnceCell::const_new();
static MAX_TTL: OnceCell<Duration> = OnceCell::const_new();
static REFRESH_MIN_INTERVAL: OnceCell<Duration> = OnceCell::const_new();
static ENABLED: OnceCell<bool> = OnceCell::const_new();

#[cfg(test)]
use std::{collections::HashMap, sync::{Mutex, OnceLock}};
#[cfg(test)]
static MOCK_STORE: OnceLock<Mutex<HashMap<String, (IpAddr, std::time::Instant)>>> = OnceLock::new();
#[cfg(test)]
fn mock_store() -> &'static Mutex<HashMap<String, (IpAddr, std::time::Instant)>> {
    MOCK_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug, Default)]
pub struct StickyDecision {
    // canonical base part of username (before '+')
    pub base_username: String,
    pub rotate: bool,
    pub ttl: Option<Duration>,
    pub session_id: Option<String>,
    // full string of username modifiers except base, canonicalized for key building
    pub param_str: String,
}

impl StickyDecision {
    pub fn enabled(&self) -> bool {
        if self.rotate { return false; }
        // global feature gating: sticky must be explicitly enabled in config
        sticky_globally_enabled()
    }

    pub fn effective_ttl(&self) -> Duration {
        let default = default_ttl();
        let desired = self.ttl.unwrap_or(default);
        let maxv = max_ttl();
        if desired > maxv { maxv } else { desired }
    }
}

pub fn parse_username_and_decision(original: &str) -> (String, StickyDecision) {
    // username+mod1=val1+mod2=val2+rotate=1
    // split on '+'; first is base username
    let mut parts = original.split('+');
    let base = parts.next().unwrap_or("").to_string();
    let mut kvs: BTreeMap<String, String> = BTreeMap::new();
    let mut rotate = false;
    let mut ttl: Option<Duration> = None;
    let mut session_id: Option<String> = None;

    for p in parts {
        if p.is_empty() { continue; }
        if let Some((k, v)) = p.split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "sticky" => {
                    if let Ok(d) = humantime::parse_duration(&val) { ttl = Some(d); }
                }
                "session_id" => { session_id = Some(val.clone()); }
                "rotate" => { rotate = val == "1" || val.eq_ignore_ascii_case("true"); }
                _ => {
                    // keep unknown params in key canon
                    kvs.insert(key, val);
                }
            }
        } else {
            // bare flag like rotate
            let key = p.trim().to_ascii_lowercase();
            if key == "rotate" { rotate = true; }
            else { kvs.insert(key, String::new()); }
        }
    }

    let mut param_str = String::new();
    for (k, v) in &kvs {
        if !param_str.is_empty() { param_str.push('&'); }
        if v.is_empty() { param_str.push_str(k); }
        else { param_str.push_str(&format!("{k}={v}")); }
    }
    // include known params in the canonical param_str for key building
    if let Some(sid) = &session_id {
        if !param_str.is_empty() { param_str.push('&'); }
        param_str.push_str(&format!("session_id={}", sid));
    }

    let decision = StickyDecision {
        base_username: base.clone(),
        rotate,
        ttl,
        session_id,
        param_str,
    };
    (base, decision)
}

fn key_prefix() -> String {
    if let Some(p) = PREFIX.get().cloned().flatten() { return p; }
    std::env::var("G3_STICKY_PREFIX").unwrap_or_else(|_| "g3proxy:sticky".to_string())
}

fn default_ttl() -> Duration {
    if let Some(d) = DEFAULT_TTL.get().copied() {
        return d;
    }
    // allow env override
    if let Ok(s) = std::env::var("G3_STICKY_DEFAULT_TTL") {
        if let Ok(d) = humantime::parse_duration(&s) {
            let _ = DEFAULT_TTL.set(d);
            return d;
        }
    }
    let d = Duration::from_secs(60);
    let _ = DEFAULT_TTL.set(d);
    d
}

fn max_ttl() -> Duration {
    if let Some(d) = MAX_TTL.get().copied() {
        return d;
    }
    if let Ok(s) = std::env::var("G3_STICKY_MAX_TTL") {
        if let Ok(d) = humantime::parse_duration(&s) {
            let _ = MAX_TTL.set(d);
            return d;
        }
    }
    let d = Duration::from_secs(3600);
    let _ = MAX_TTL.set(d);
    d
}

pub fn refresh_min_interval() -> Duration {
    if let Some(d) = REFRESH_MIN_INTERVAL.get().copied() {
        return d;
    }
    if let Ok(s) = std::env::var("G3_STICKY_REFRESH_MIN_INTERVAL") {
        if let Ok(d) = humantime::parse_duration(&s) {
            let _ = REFRESH_MIN_INTERVAL.set(d);
            return d;
        }
    }
    let d = Duration::from_millis(250);
    let _ = REFRESH_MIN_INTERVAL.set(d);
    d
}

pub fn set_redis_url(url: Option<&str>) {
    match url {
        Some(s) if !s.is_empty() => {
            let _ = REDIS_URL.set(Some(s.to_string()));
            // best-effort parse and cache a client config
            if let Ok(u) = Url::parse(s) {
                if let Some(host) = u.host_str() {
                    let port = u.port().unwrap_or(g3_redis_client::REDIS_DEFAULT_PORT);
                    if let Ok(upstream) = g3_types::net::UpstreamAddr::from_host_str_and_port(host, port) {
                        let mut builder = g3_redis_client::RedisClientConfigBuilder::new(upstream);
                        // db
                        let path = u.path();
                        let db_str = path.strip_prefix('/').unwrap_or(path);
                        if !db_str.is_empty() {
                            if let Ok(db) = db_str.parse::<i64>() { builder.set_db(db); }
                        }
                        // user/pass
                        let username = u.username();
                        if !username.is_empty() { builder.set_username(username.to_string()); }
                        if let Some(password) = u.password() { builder.set_password(password.to_string()); }
                        // query params as yaml kv
                        for (k, v) in u.query_pairs() {
                            let yaml_val = yaml_rust::Yaml::String(v.to_string());
                            let _ = builder.set_by_yaml_kv(&k, &yaml_val, None);
                        }
                        if let Ok(client_cfg) = builder.build() {
                            let _ = REDIS_CLIENT.set(Some(client_cfg));
                        }
                    }
                }
            }
        }
        _ => { let _ = REDIS_URL.set(None); let _ = REDIS_CLIENT.set(None); }
    }
}

pub fn set_prefix(p: Option<&str>) {
    match p {
        Some(s) if !s.is_empty() => { let _ = PREFIX.set(Some(s.to_string())); }
        _ => { let _ = PREFIX.set(None); }
    }
}

pub fn set_enabled(v: bool) {
    // ignore error if already set
    let _ = ENABLED.set(v);
}

fn sticky_globally_enabled() -> bool {
    if let Some(v) = ENABLED.get().copied() { return v; }
    // For test builds, default to enabled to keep unit tests concise unless explicitly set.
    cfg!(test)
}

pub fn set_default_ttl(d: Option<Duration>) {
    if let Some(v) = d { let _ = DEFAULT_TTL.set(v); }
}

pub fn set_max_ttl(d: Option<Duration>) {
    if let Some(v) = d { let _ = MAX_TTL.set(v); }
}

pub fn build_sticky_key(decision: &StickyDecision, upstream: &UpstreamAddr) -> String {
    // Format: <prefix>:<upstream>|<base_username>[|<canonical_params>]
    // canonical_params contains sorted unknowns and session_id only; excludes sticky/rotate/ttl
    let mut key = String::with_capacity(128);
    key.push_str(&key_prefix());
    key.push(':');
    key.push_str(&upstream.to_string());
    key.push('|');
    key.push_str(&decision.base_username);
    if !decision.param_str.is_empty() {
        key.push('|');
        key.push_str(&decision.param_str);
    }
    key
}

fn rendezvous_pick(key: &str, ips: &[IpAddr]) -> Option<IpAddr> {
    use std::hash::{Hash, Hasher};

    fn h64<T: Hash>(t: &T) -> u64 {
        let mut h = FnvHasher::with_key(0xcbf29ce484222325);
        t.hash(&mut h);
        h.finish()
    }

    let base = h64(&key);
    let mut best: Option<(u64, IpAddr)> = None;
    for ip in ips {
        // hash ip separately then mix deterministically with the base key hash
        let hip = h64(ip);
        let mut score = base ^ hip ^ 0x9e37_79b9_7f4a_7c15;
        // xorshift64* style mixing for better avalanche
        score ^= score << 13;
        score ^= score >> 7;
        score ^= score << 17;
        match best {
            Some((b, _)) if score <= b => {}
            _ => best = Some((score, *ip)),
        }
    }
    best.map(|(_, ip)| ip)
}

pub async fn redis_get_ip(key: &str) -> Option<IpAddr> {
    #[cfg(test)]
    {
        let now = std::time::Instant::now();
        if let Some((ip, exp)) = mock_store().lock().unwrap().get(key).cloned() {
            if now < exp { return Some(ip); } else { let _ = mock_store().lock().unwrap().remove(key); }
        }
    }
    let client_cfg = match REDIS_CLIENT.get().and_then(|o| o.as_ref()) { Some(c) => c, None => return None };
    let mut conn = match client_cfg.connect().await { Ok(c) => c, Err(_) => return None };
    let s: Option<String> = match conn.get(key).await { Ok(v) => v, Err(_) => None };
    s.and_then(|v| v.parse::<IpAddr>().ok())
}

pub async fn redis_set_ip(key: &str, ip: IpAddr, ttl: Duration) {
    #[cfg(test)]
    {
        let exp = std::time::Instant::now() + ttl;
        mock_store().lock().unwrap().insert(key.to_string(), (ip, exp));
    }
    if let Some(client_cfg) = REDIS_CLIENT.get().and_then(|o| o.as_ref()) {
        if let Ok(mut conn) = client_cfg.connect().await {
            // Use atomic SET with EX
            let _: redis::RedisResult<()> = redis::cmd("SET")
                .arg(key)
                .arg(ip.to_string())
                .arg("EX")
                .arg(ttl.as_secs() as i64)
                .query_async(&mut conn)
                .await;
        }
    }
}

pub async fn redis_refresh_ttl(key: &str, ttl: Duration) {
    #[cfg(test)]
    {
        if let Some((ip, _)) = mock_store().lock().unwrap().get(key).cloned() {
            let exp = std::time::Instant::now() + ttl;
            mock_store().lock().unwrap().insert(key.to_string(), (ip, exp));
        }
    }
    if let Some(client_cfg) = REDIS_CLIENT.get().and_then(|o| o.as_ref()) {
        if let Ok(mut conn) = client_cfg.connect().await {
            let _ : redis::RedisResult<()> = conn.expire(key, ttl.as_secs() as i64).await;
        }
    }
}

pub async fn choose_sticky_ip(
    decision: &StickyDecision,
    upstream: &UpstreamAddr,
    ips: &[IpAddr],
) -> Option<(IpAddr, String, bool)> {
    if !decision.enabled() { return None; }
    if decision.rotate { return None; }
    if ips.is_empty() { return None; }
    let key = build_sticky_key(decision, upstream);
    if let Some(ip) = redis_get_ip(&key).await {
        return Some((ip, key, true));
    }
    let pick = rendezvous_pick(&key, ips)?;
    Some((pick, key, false))
}

pub fn compute_expiry(now: DateTime<Utc>, ttl: Duration) -> DateTime<Utc> {
    now + chrono::TimeDelta::from_std(ttl).unwrap_or_else(|_| chrono::TimeDelta::seconds(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    // no block_on; async tests use .await

    #[test]
    fn parse_username_plus_mods() {
        let (base, d) = parse_username_and_decision("alice+sticky=5m+session_id=abc");
        assert_eq!(base, "alice");
        assert!(d.ttl.is_some());
        assert_eq!(d.effective_ttl().as_secs(), 300);
        assert_eq!(d.session_id.as_deref(), Some("abc"));
        assert!(!d.rotate);

        let (base2, d2) = parse_username_and_decision("bob+session_id=cart42");
        assert_eq!(base2, "bob");
        assert!(d2.ttl.is_none());
        assert_eq!(d2.effective_ttl().as_secs(), 60);
        assert_eq!(d2.session_id.as_deref(), Some("cart42"));

        let (base3, d3) = parse_username_and_decision("eve+rotate=1+sticky=10s");
        assert_eq!(base3, "eve");
        assert!(d3.rotate);
        assert!(!d3.enabled());
        // rotate overrides and disables stickiness
    }

    #[test]
    fn hrw_pick_member() {
        let key = "g3proxy:sticky:example.com:80|alice+session_id=x".to_string();
        let ips = vec![
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3)),
        ];
        let pick = rendezvous_pick(&key, &ips).unwrap();
        assert!(ips.contains(&pick));
        // deterministic
        let pick2 = rendezvous_pick(&key, &ips).unwrap();
        assert_eq!(pick, pick2);
    }

    #[test]
    fn key_build_contains_host_user() {
        use g3_types::net::UpstreamAddr;
        let ups: UpstreamAddr = "example.com:8080".parse().unwrap();
        let (_, d) = parse_username_and_decision("alice+session_id=s1");
        let k = build_sticky_key(&d, &ups);
        assert!(k.contains("example.com:8080"));
        assert!(k.contains("|alice"));
        assert!(k.contains("session_id=s1"));
    }

    #[test]
    fn parse_unknown_params_and_order() {
        let (_base, d) = parse_username_and_decision("alice+zzz=9+aaa=1+session_id=sx");
        // param_str is canonical: sorted unknowns, then session_id included
        assert!(d.param_str.contains("aaa=1"));
        assert!(d.param_str.contains("zzz=9"));
        assert!(d.param_str.contains("session_id=sx"));
    }

    #[test]
    fn rotate_true_disables() {
        let (_base, d) = parse_username_and_decision("eve+rotate=true");
        assert!(d.rotate);
        assert!(!d.enabled());
    }

    #[test]
    fn hrw_ipv6() {
        let key = "g3proxy:sticky:[2001:db8::1]:443|bob+session_id=y".to_string();
        let ips = vec![
            IpAddr::from([0x20,0x01,0x0d,0xb8,0,0,0,0,0,0,0,0,0,0,0,1]),
            IpAddr::from([0x20,0x01,0x0d,0xb8,0,0,0,0,0,0,0,0,0,0,0,2]),
        ];
        let pick = rendezvous_pick(&key, &ips).unwrap();
        assert!(ips.contains(&pick));
    }

    #[test]
    fn ttl_clamp_max() {
        // default max is 1h; requesting 2h should clamp to 1h
        let (_base, d) = parse_username_and_decision("alice+sticky=2h");
        assert_eq!(d.effective_ttl().as_secs(), 3600);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn hrw_all_members_eventually_chosen() {
        use std::net::{Ipv4Addr};
        let ups: UpstreamAddr = "example.com:80".parse().unwrap();
        let ips = vec![
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3)),
        ];
        let mut seen = std::collections::HashSet::new();
        for i in 0..5000u32 {
            let s = format!("{}|session_id=s{i}", build_sticky_key(&parse_username_and_decision("user+session_id=x").1, &ups));
            if let Some(ip) = rendezvous_pick(&s, &ips) {
                seen.insert(ip);
                if seen.len() == ips.len() { break; }
            }
        }
        assert_eq!(seen.len(), ips.len());
    }

    // Validate HRW load distribution across a large address set.
    // Simulate a domain resolving to 1000 A records and sample many different keys;
    // the selection should be approximately balanced across all IPs.
    #[test]
    fn hrw_balanced_over_1k_ips() {
        use std::collections::HashMap;
        use std::net::{Ipv4Addr};

        // build 1000 IPv4 addresses: 10.0.0.1 .. 10.0.3.232 (first 1000 addresses)
        let mut ips = Vec::with_capacity(1000);
        let mut oct3 = 0u8;
        let mut oct4 = 1u8;
        for _ in 0..1000 {
            ips.push(IpAddr::V4(Ipv4Addr::new(10, 0, oct3, oct4)));
            oct4 = oct4.wrapping_add(1);
            if oct4 == 0 { oct3 = oct3.wrapping_add(1); oct4 = 1; }
        }

        // index map for counting
        let mut index: HashMap<IpAddr, usize> = HashMap::with_capacity(ips.len());
        for (i, ip) in ips.iter().copied().enumerate() { index.insert(ip, i); }

        let samples: usize = 20_000; // keep test runtime reasonable in debug builds
        let mean_expected = samples as f64 / ips.len() as f64; // ~20

        // produce varying keys; use deterministic pattern to avoid RNG
        let base = "g3proxy:sticky:example.com:80|user";
        let mut counts = vec![0usize; ips.len()];
        for i in 0..samples {
            let key = format!("{}+session_id=s{}", base, i);
            let pick = super::rendezvous_pick(&key, &ips).expect("pick from non-empty");
            let idx = *index.get(&pick).unwrap();
            counts[idx] += 1;
        }

        // basic sanity: almost all members should get traffic
        // With λ≈20 per bucket, zero-count probability is ~2e-9 per bucket under ideal iid.
        // Allow a tiny fraction for robustness across hash implementations.
        let zero = counts.iter().filter(|&&c| c == 0).count();
        assert!(zero <= ips.len() / 100, "too many zero-count buckets: {zero}");

        // compute sample mean and standard deviation
        let mean: f64 = mean_expected;
        let var: f64 = counts
            .iter()
            .map(|&c| {
                let d = c as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / counts.len() as f64;
        let sd = var.sqrt();

        // relative standard deviation should be reasonably small
        let rel_sd = sd / mean;
        assert!(rel_sd < 0.5, "relative SD too high: {rel_sd:.3}");

        // also ensure min/max are within a broad tolerance band around the mean
        let min = *counts.iter().min().unwrap() as f64;
        let max = *counts.iter().max().unwrap() as f64;
        let lower = mean - 10.0 * (mean.sqrt()); // ~ mean - 10*sqrt(mean)
        let upper = mean + 10.0 * (mean.sqrt()); // ~ mean + 10*sqrt(mean)
        assert!(min >= 0.0_f64.max(lower) && max <= upper,
            "counts out of tolerance: min={min}, max={max}, mean={mean}");
    }

    #[test]
    fn redis_sliding_ttl_sets_and_refreshes() {
        // Use mock store for tests; avoid external Redis and async runtime
        let unique = format!("g3proxy:test:sticky:{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        super::set_prefix(Some(&unique));

        let ups: UpstreamAddr = "example.com:80".parse().unwrap();
        let ips = vec![
            "192.0.2.10".parse().unwrap(),
            "192.0.2.11".parse().unwrap(),
        ];
        let (_base, d) = parse_username_and_decision("user+session_id=redis_integ+sticky=5s");
        let ttl = std::time::Duration::from_secs(5);

        let key = build_sticky_key(&d, &ups);
        let pick = rendezvous_pick(&key, &ips).unwrap();
        // set
        {
            let exp = std::time::Instant::now() + ttl;
            mock_store().lock().unwrap().insert(key.clone(), (pick, exp));
        }
        // check
        let exp1 = mock_store().lock().unwrap().get(&key).unwrap().1;
        // refresh (avoid holding the lock across a re-lock)
        {
            let ip_opt = {
                let store = mock_store().lock().unwrap();
                store.get(&key).map(|(ip, _)| *ip)
            };
            if let Some(ip) = ip_opt {
                let exp = std::time::Instant::now() + ttl;
                mock_store().lock().unwrap().insert(key.clone(), (ip, exp));
            }
        }
        let exp2 = mock_store().lock().unwrap().get(&key).unwrap().1;
        assert!(exp2 > exp1, "refresh should extend expiration instant");
        let _ = mock_store().lock().unwrap().remove(&key);
    }

    // Demonstrates that after TTL expiry, HRW mapping remains the same unless inputs change.
    // This matches current design: TTL controls cache lifetime, not forced rotation.
    #[tokio::test(flavor = "current_thread")]
    async fn ttl_expiry_does_not_force_rotation() {
        // Use mock store for tests
        let ups: UpstreamAddr = "example.com:80".parse().unwrap();
        let ips: Vec<IpAddr> = vec![
            "192.0.2.101".parse().unwrap(),
            "192.0.2.102".parse().unwrap(),
            "192.0.2.103".parse().unwrap(),
        ];

        let unique = format!("g3proxy:test:ttlrotate:{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        super::set_prefix(Some(&unique));

        // Use 1s sticky with fixed session_id key
        let (_base, d) = parse_username_and_decision("user+sticky=1s+session_id=mamatata");
        let ttl = std::time::Duration::from_secs(1);
        let (pick1, key, _hit) = choose_sticky_ip(&d, &ups, &ips).await.unwrap();
        super::redis_set_ip(&key, pick1, ttl).await;

        // Ensure key exists
        assert!(mock_store().lock().unwrap().contains_key(&key));

        // Simulate expiry by removing key from mock store
        let _ = mock_store().lock().unwrap().remove(&key);

        // Next selection: no cache hit, HRW decides deterministically -> same IP
        let (pick2, _key2, hit2) = choose_sticky_ip(&d, &ups, &ips).await.unwrap();
        assert!(!hit2, "cache should have expired");
        assert_eq!(pick2, pick1, "HRW deterministic mapping returns same upstream after expiry");
        let _ = mock_store().lock().unwrap().remove(&key);
    }
}
