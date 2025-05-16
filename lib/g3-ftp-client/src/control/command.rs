/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::LimitedWriteExt;

use super::FtpControlChannel;

#[derive(Debug, Clone, Copy)]
pub struct FtpCommand(&'static str);

impl fmt::Display for FtpCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

macro_rules! ftp_commands {
    (
        $(
            $(#[$docs:meta])*
            ($konst:ident, $phrase:expr);
        )+
    ) => {
        impl FtpCommand {
        $(
            $(#[$docs])*
            pub const $konst: FtpCommand = FtpCommand($phrase);
        )+
        }
    };
}

ftp_commands! {
    /// a fake command for greeting
    (GREETING, "-");
    (SPDT, "SPDT");
    (FEAT, "FEAT");
    (OPTS_UTF8_ON, "OPTS UTF8 ON");
    (USER, "USER");
    (PASS, "PASS");
    (QUIT, "QUIT");
    (DELE, "DELE");
    (RMD, "RMD");
    (TYPE_A, "TYPE A");
    (TYPE_I, "TYPE I");
    (PASV, "PASV");
    (EPSV, "EPSV");
    (SPSV, "SPSV");
    (MLST, "MLST");
    (SIZE, "SIZE");
    (MDTM, "MDTM");
    (ABOR, "ABOR");
    (PRET, "PRET");
    (LIST, "LIST");
    (REST, "REST");
    (RETR, "RETR");
    (STOR, "STOR");
}

impl<T> FtpControlChannel<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    async fn send_all(&mut self, buf: &[u8]) -> io::Result<()> {
        #[cfg(feature = "log-raw-io")]
        crate::debug::log_cmd(unsafe { std::str::from_utf8_unchecked(buf).trim_end() });

        self.stream.write_all_flush(buf).await?;
        Ok(())
    }

    pub(super) async fn send_cmd(&mut self, cmd: FtpCommand) -> io::Result<()> {
        let len = cmd.0.len() + 2;
        let mut buf: Vec<u8> = Vec::with_capacity(len);
        buf.extend_from_slice(cmd.0.as_bytes());
        buf.extend_from_slice(b"\r\n");

        self.send_all(buf.as_ref()).await
    }

    pub(super) async fn send_cmd1(&mut self, cmd: FtpCommand, param1: &str) -> io::Result<()> {
        let len = cmd.0.len() + 1 + param1.len() + 2;
        let mut buf: Vec<u8> = Vec::with_capacity(len);
        buf.extend_from_slice(cmd.0.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(param1.as_bytes());
        buf.extend_from_slice(b"\r\n");

        self.send_all(buf.as_ref()).await
    }

    pub(super) async fn send_pre_transfer_cmd1(
        &mut self,
        cmd: FtpCommand,
        param1: &str,
    ) -> io::Result<()> {
        let len = 5 + cmd.0.len() + 1 + param1.len() + 2;
        let mut buf: Vec<u8> = Vec::with_capacity(len);
        buf.extend_from_slice(b"PRET ");
        buf.extend_from_slice(cmd.0.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(param1.as_bytes());
        buf.extend_from_slice(b"\r\n");

        self.send_all(buf.as_ref()).await
    }
}
