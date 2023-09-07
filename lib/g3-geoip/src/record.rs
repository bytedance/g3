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

use ip_network::IpNetwork;

use super::{ContinentCode, CountryCode};

pub struct GeoIpCountryRecord {
    pub network: IpNetwork,
    pub country: CountryCode,
    pub continent: ContinentCode,
}

pub struct GeoIpAsnRecord {
    pub network: IpNetwork,
    pub number: u32,
    pub(crate) name: Option<String>,
    pub(crate) domain: Option<String>,
}

impl GeoIpAsnRecord {
    pub fn isp_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn isp_domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
}
