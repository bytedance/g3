/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::net::SocketAddr;

use hickory_proto::ProtoError;
use tokio::net::{TcpSocket, TcpStream};

pub(crate) async fn tcp_connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
) -> Result<TcpStream, ProtoError> {
    let socket = match name_server {
        SocketAddr::V4(_) => TcpSocket::new_v4(),
        SocketAddr::V6(_) => TcpSocket::new_v6(),
    }?;
    if let Some(addr) = bind_addr {
        socket.bind(addr)?;
    }

    let tcp_stream = socket.connect(name_server).await?;
    Ok(tcp_stream)
}
