/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use async_trait::async_trait;
use hickory_proto::tcp::{Connect, DnsTcpStream};

pub mod rustls;

#[async_trait]
pub trait TlsConnect<S: Connect> {
    type TlsStream: DnsTcpStream;

    fn server_name(&self) -> String;

    async fn tls_connect(&self, stream: S) -> io::Result<Self::TlsStream>;
}
