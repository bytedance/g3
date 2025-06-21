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

#[cfg(any(target_os = "linux", target_os = "openbsd"))]
pub(super) fn default() -> io::Result<UnixDatagram> {
    connect("/dev/log")
}

#[cfg(any(target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd"))]
pub(super) fn default() -> io::Result<UnixDatagram> {
    connect("/var/run/log")
}

#[cfg(target_os = "macos")]
pub(super) fn default() -> io::Result<UnixDatagram> {
    connect("/var/run/syslog")
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
)))]
pub(super) fn default() -> io::Result<UnixDatagram> {
    log::warn!("no default syslog path known on this platform, will try /dev/log");
    connect("/dev/log")
}

pub(super) fn custom<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
    connect(path)
}
