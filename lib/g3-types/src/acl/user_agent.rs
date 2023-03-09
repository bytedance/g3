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

use std::collections::BTreeMap;

use super::AclAction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclUserAgentRule {
    inner: BTreeMap<String, AclAction>,
    missed_action: AclAction,
}

impl Default for AclUserAgentRule {
    fn default() -> Self {
        // default to permit all
        AclUserAgentRule::new(AclAction::Permit)
    }
}

impl AclUserAgentRule {
    pub fn new(missed_action: AclAction) -> Self {
        AclUserAgentRule {
            inner: BTreeMap::new(),
            missed_action,
        }
    }

    pub fn add_ua_name(&mut self, ua: &str, action: AclAction) {
        let name = ua.to_ascii_lowercase();
        self.inner.insert(name, action);
    }

    #[inline]
    pub fn missed_action(&self) -> AclAction {
        self.missed_action
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.missed_action = action;
    }

    pub fn check(&self, ua_value: &str) -> (bool, AclAction) {
        let value = ua_value.to_ascii_lowercase();

        for (name, action) in self.inner.iter() {
            let mut offset = 0usize;

            while offset < value.len() {
                let vs = &value[offset..];

                if let Some(pos) = vs.find(name) {
                    let pos_pre = offset + pos;
                    if pos_pre != 0 && !matches!(value.as_bytes()[pos_pre - 1], b' ' | b';') {
                        // just skip the current match and offset one more byte,
                        // as the UserAgent name should not contain ' ' or ';'
                        offset += name.len() + 1;
                        continue;
                    }

                    let pos_next = offset + pos + name.len();
                    if pos_next < value.len()
                        && !matches!(value.as_bytes()[pos_next], b'/' | b' ' | b';')
                    {
                        // just skip the current match and offset one more byte,
                        // as the UserAgent name should not contain ' ' or ';'
                        offset += name.len();
                        continue;
                    }

                    return (true, *action);
                } else {
                    break;
                }
            }
        }

        (false, self.missed_action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut acl = AclUserAgentRule::default();
        acl.add_ua_name("curl", AclAction::Forbid);
        acl.add_ua_name("go", AclAction::Forbid);

        let (found, action) = acl.check("curl/7.74.0");
        assert!(found);
        assert_eq!(action, AclAction::Forbid);

        let (found, action) = acl.check("AdsBot-Google (+http://www.google.com/adsbot.html)");
        assert!(!found);
        assert_eq!(action, AclAction::Permit);

        let (found, action) =
            acl.check("Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)");
        assert!(!found);
        assert_eq!(action, AclAction::Permit);
    }

    #[test]
    fn google_bots_apis() {
        let mut acl = AclUserAgentRule::default();
        acl.add_ua_name("APIs-Google", AclAction::Forbid);
        let (found, action) =
            acl.check("APIs-Google (+https://developers.google.com/webmasters/APIs-Google.html)");
        assert!(found);
        assert_eq!(action, AclAction::Forbid);
    }

    #[test]
    fn google_bots_ads_mobile() {
        let mut acl = AclUserAgentRule::default();
        acl.add_ua_name("AdsBot-Google-Mobile", AclAction::Forbid);

        let (found, action) = acl.check(
            "\
            Mozilla/5.0 (Linux; Android 5.0; SM-G920A) \
            AppleWebKit (KHTML, like Gecko) \
            Chrome \
            Mobile \
            Safari (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)",
        );
        assert!(found);
        assert_eq!(action, AclAction::Forbid);

        let (found, action) = acl.check(
            "\
            Mozilla/5.0 (iPhone; CPU iPhone OS 9_1 like Mac OS X) \
            AppleWebKit/601.1.46 (KHTML, like Gecko) \
            Version/9.0 \
            Mobile/13B143 \
            Safari/601.1 (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)",
        );
        assert!(found);
        assert_eq!(action, AclAction::Forbid);
    }
}
