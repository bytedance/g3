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

use anyhow::anyhow;
use openssl::nid::Nid;
use openssl::x509::{X509Name, X509NameBuilder};

#[derive(Default)]
pub struct SubjectNameBuilder {
    organization: Option<String>,
    organization_unit: Option<String>,
    country: Option<String>,
}

impl SubjectNameBuilder {
    pub fn set_organization(&mut self, o: String) {
        self.organization = Some(o);
    }

    pub fn set_organization_unit(&mut self, ou: String) {
        self.organization_unit = Some(ou);
    }

    pub fn set_country(&mut self, c: String) {
        self.country = Some(c);
    }

    fn get_builder(&self) -> anyhow::Result<X509NameBuilder> {
        let mut builder = X509Name::builder()
            .map_err(|e| anyhow!("failed to create x509 subject name builder: {e}"))?;
        if let Some(o) = &self.organization {
            builder
                .append_entry_by_nid(Nid::ORGANIZATIONNAME, o)
                .map_err(|e| anyhow!("failed to set organization name to {o}: {e}"))?;
        }
        if let Some(ou) = &self.organization_unit {
            builder
                .append_entry_by_nid(Nid::ORGANIZATIONALUNITNAME, ou)
                .map_err(|e| anyhow!("failed to set organization unit name to {ou}: {e}"))?;
        }
        if let Some(c) = &self.country {
            builder
                .append_entry_by_nid(Nid::COUNTRYNAME, c)
                .map_err(|e| anyhow!("failed to set country name to {c}: {e}"))?;
        }
        Ok(builder)
    }

    pub(super) fn build_with_common_name(&self, cn: &str) -> anyhow::Result<X509Name> {
        let mut builder = self.get_builder()?;
        builder
            .append_entry_by_nid(Nid::COMMONNAME, cn)
            .map_err(|e| anyhow!("failed to set common name to {cn}: {e}"))?;
        Ok(builder.build())
    }

    pub(super) fn build_for_user(&self, uid: &str) -> anyhow::Result<X509Name> {
        let mut builder = self.get_builder()?;
        builder
            .append_entry_by_nid(Nid::USERID, uid)
            .map_err(|e| anyhow!("failed to set user identifier to {uid}: {e}"))?;
        Ok(builder.build())
    }

    #[allow(unused)]
    pub(super) fn build_for_mail(&self, mail: &str) -> anyhow::Result<X509Name> {
        let mut builder = self.get_builder()?;
        builder
            .append_entry_by_nid(Nid::MAIL, mail)
            .map_err(|e| anyhow!("failed to set email address to {mail}: {e}"))?;
        Ok(builder.build())
    }
}
