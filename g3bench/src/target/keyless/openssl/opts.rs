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

use anyhow::Context;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::target::keyless::opts::KeylessAction;
use crate::target::keyless::{AppendKeylessArgs, KeylessGlobalArgs};

const ARG_DUMP_RESULT: &str = "dump-result";

pub(super) struct KeylessOpensslArgs {
    global: KeylessGlobalArgs,
    dump_result: bool,
}

impl KeylessOpensslArgs {
    pub(super) fn handle_action(&self) -> anyhow::Result<Vec<u8>> {
        match self.global.action {
            KeylessAction::RsaDecrypt(padding) => self.global.rsa_decrypt(padding),
            KeylessAction::RsaSign(digest) => self.global.pkey_sign(digest),
            KeylessAction::EcdsaSign(digest) => self.global.pkey_sign(digest),
        }
    }

    pub(super) fn dump_result(&self, task_id: usize, data: Vec<u8>) {
        if self.dump_result {
            let hex_str = hex::encode(data);
            println!("== Output of task {task_id}:\n{hex_str}");
        }
    }
}

pub(super) fn add_openssl_args(app: Command) -> Command {
    app.arg(
        Arg::new(ARG_DUMP_RESULT)
            .help("Dump output use hex string")
            .action(ArgAction::SetTrue)
            .num_args(0)
            .long(ARG_DUMP_RESULT),
    )
    .append_keyless_args()
}

pub(super) fn parse_openssl_args(args: &ArgMatches) -> anyhow::Result<KeylessOpensslArgs> {
    let global_args =
        KeylessGlobalArgs::parse_args(args).context("failed to parse global keyless args")?;
    let dump_result = args.get_flag(ARG_DUMP_RESULT);

    Ok(KeylessOpensslArgs {
        global: global_args,
        dump_result,
    })
}
