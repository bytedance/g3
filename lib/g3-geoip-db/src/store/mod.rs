/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, LazyLock};

use arc_swap::ArcSwapOption;
use ip_network_table::IpNetworkTable;

use crate::{GeoIpAsnRecord, GeoIpCountryRecord};

static GEO_COUNTRY_DB: LazyLock<ArcSwapOption<IpNetworkTable<GeoIpCountryRecord>>> =
    LazyLock::new(|| ArcSwapOption::new(None));
static GEO_ASN_DB: LazyLock<ArcSwapOption<IpNetworkTable<GeoIpAsnRecord>>> =
    LazyLock::new(|| ArcSwapOption::new(None));

pub fn load_country() -> Option<Arc<IpNetworkTable<GeoIpCountryRecord>>> {
    GEO_COUNTRY_DB.load_full()
}

pub fn store_country(db: Arc<IpNetworkTable<GeoIpCountryRecord>>) {
    GEO_COUNTRY_DB.store(Some(db));
}

pub fn load_asn() -> Option<Arc<IpNetworkTable<GeoIpAsnRecord>>> {
    GEO_ASN_DB.load_full()
}

pub fn store_asn(db: Arc<IpNetworkTable<GeoIpAsnRecord>>) {
    GEO_ASN_DB.store(Some(db));
}
