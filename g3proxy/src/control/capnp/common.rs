/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3proxy_proto::types_capnp::operation_result;

pub(super) fn set_operation_result(
    mut builder: operation_result::Builder<'_>,
    r: anyhow::Result<()>,
) {
    match r {
        Ok(_) => builder.set_ok("success"),
        Err(e) => {
            let mut ev = builder.init_err();
            ev.set_code(-1);
            ev.set_reason(format!("{e:?}").as_str());
        }
    }
}
