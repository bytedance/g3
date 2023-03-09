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
