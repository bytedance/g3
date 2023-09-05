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

mod error;
pub use error::{CommandError, CommandResult};

pub fn print_ok_notice(notice_reader: capnp::text::Reader<'_>) -> CommandResult<()> {
    match notice_reader.to_str() {
        Ok(notice) => {
            println!("notice: {notice}");
            Ok(())
        }
        Err(e) => Err(CommandError::Utf8 {
            field: "ok",
            reason: e,
        }),
    }
}

pub fn print_text(field: &'static str, text_reader: capnp::text::Reader<'_>) -> CommandResult<()> {
    match text_reader.to_str() {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(e) => Err(CommandError::Utf8 { field, reason: e }),
    }
}

#[inline]
pub fn print_version(version_reader: capnp::text::Reader<'_>) -> CommandResult<()> {
    print_text("version", version_reader)
}

pub fn print_text_list(
    field: &'static str,
    list: capnp::text_list::Reader<'_>,
) -> CommandResult<()> {
    for text in list.iter() {
        print_text(field, text?)?;
    }
    Ok(())
}

#[inline]
pub fn print_result_list(result_list_reader: capnp::text_list::Reader<'_>) -> CommandResult<()> {
    print_text_list("result", result_list_reader)
}

pub fn print_data(data_reader: capnp::data::Reader<'_>) {
    println!("{}", hex::encode(data_reader));
}

pub fn print_data_list(list: capnp::data_list::Reader<'_>) -> CommandResult<()> {
    for data in list.iter() {
        print_data(data?);
    }
    Ok(())
}
