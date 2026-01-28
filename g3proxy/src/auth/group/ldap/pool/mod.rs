/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use tokio::sync::oneshot;

use g3_types::auth::{Password, Username};

mod connect;
pub(super) use connect::LdapConnector;

mod task;
use task::LdapAuthTask;

struct LdapAuthRequest {
    uid: Username,
    password: Password,
    result_sender: oneshot::Sender<Option<(Username, Password)>>,
}
