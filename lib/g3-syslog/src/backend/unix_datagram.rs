/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::path::Path;

use std::os::unix::net::UnixDatagram;

fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
    let sock = UnixDatagram::unbound()?;
    sock.connect(path)?;
    Ok(sock)
}

pub(super) fn default() -> io::Result<UnixDatagram> {
    connect("/dev/log").or_else(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            connect("/var/run/syslog")
        } else {
            Err(e)
        }
    })
}

pub(super) fn custom<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
    connect(path)
}
