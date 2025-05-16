/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use super::{CommandError, CommandResult};

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
