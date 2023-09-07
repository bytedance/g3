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

use arc_swap::ArcSwapOption;
use ip_network_table::IpNetworkTable;
use once_cell::sync::Lazy;

use crate::{GeoIpAsRecord, GeoIpCountryRecord};

static GEO_COUNTRY_DB: Lazy<ArcSwapOption<IpNetworkTable<GeoIpCountryRecord>>> =
    Lazy::new(|| ArcSwapOption::new(None));
static GEO_AS_DB: Lazy<ArcSwapOption<IpNetworkTable<GeoIpAsRecord>>> =
    Lazy::new(|| ArcSwapOption::new(None));

pub fn load_country() -> Option<Arc<IpNetworkTable<GeoIpCountryRecord>>> {
    GEO_COUNTRY_DB.load_full()
}

pub fn store_country(db: Arc<IpNetworkTable<GeoIpCountryRecord>>) {
    GEO_COUNTRY_DB.store(Some(db));
}

pub fn load_as() -> Option<Arc<IpNetworkTable<GeoIpAsRecord>>> {
    GEO_AS_DB.load_full()
}

pub fn store_as(db: Arc<IpNetworkTable<GeoIpAsRecord>>) {
    GEO_AS_DB.store(Some(db));
}
