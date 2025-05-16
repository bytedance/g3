/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::Context;
use clap::{ArgMatches, Command};

use crate::target::keyless::opts::KeylessAction;
use crate::target::keyless::{AppendKeylessArgs, KeylessGlobalArgs};

pub(super) struct KeylessOpensslArgs {
    pub(super) global: KeylessGlobalArgs,
}

impl KeylessOpensslArgs {
    pub(super) fn handle_action(&self) -> anyhow::Result<Vec<u8>> {
        match self.global.action {
            KeylessAction::RsaSign(digest, padding) => self.global.sign_rsa(digest, padding),
            KeylessAction::EcdsaSign(digest) => self.global.sign(digest),
            KeylessAction::Ed25519Sign => self.global.sign_ed(),
            KeylessAction::RsaDecrypt(padding) => self.global.decrypt_rsa(padding),
            KeylessAction::RsaEncrypt(padding) => self.global.encrypt_rsa(padding),
            KeylessAction::Decrypt => self.global.decrypt(),
            KeylessAction::Encrypt => self.global.encrypt(),
            KeylessAction::RsaPrivateEncrypt(padding) => self.global.rsa_private_encrypt(padding),
            KeylessAction::RsaPublicDecrypt(padding) => self.global.rsa_public_decrypt(padding),
        }
    }
}

pub(super) fn add_openssl_args(app: Command) -> Command {
    app.append_keyless_args()
}

pub(super) fn parse_openssl_args(args: &ArgMatches) -> anyhow::Result<KeylessOpensslArgs> {
    let global_args =
        KeylessGlobalArgs::parse_args(args).context("failed to parse global keyless args")?;

    Ok(KeylessOpensslArgs {
        global: global_args,
    })
}
