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

use g3_ctl::{CommandError, CommandResult};

use g3keymess_proto::types_capnp::{fetch_result, operation_result};

pub(crate) fn parse_operation_result(r: operation_result::Reader<'_>) -> CommandResult<()> {
    match r.which().unwrap() {
        operation_result::Which::Ok(ok) => g3_ctl::print_ok_notice(ok?),
        operation_result::Which::Err(err) => {
            let e = err?;
            Err(CommandError::api_error(e.get_code(), e.get_reason()?))
        }
    }
}

pub(crate) fn parse_fetch_result<T>(
    r: fetch_result::Reader<'_, T>,
) -> CommandResult<<T as capnp::traits::Owned>::Reader<'_>>
where
    T: capnp::traits::Owned,
{
    match r.which().unwrap() {
        fetch_result::Which::Data(data) => Ok(data?),
        fetch_result::Which::Err(err) => {
            let e = err?;
            Err(CommandError::api_error(e.get_code(), e.get_reason()?))
        }
    }
}
