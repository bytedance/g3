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

use log::warn;
use tokio::io::{AsyncBufRead, AsyncWrite};

mod local;
pub use local::LocalController;

pub mod bridge;
pub mod capnp;

pub mod config;
use config::{GeneralControllerConfig, LocalControllerConfig};

mod text;

#[derive(Eq, PartialEq)]
pub(crate) enum CtlProtoType {
    End,
    Text,
    CapnP,
}

pub(crate) struct CtlProtoCtx<R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    reader: R,
    writer: W,
    config: GeneralControllerConfig,
    protocol_type: CtlProtoType,
}

impl<R, W> CtlProtoCtx<R, W>
where
    R: AsyncBufRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    pub(crate) fn new(
        reader: R,
        writer: W,
        config: GeneralControllerConfig,
        protocol_type: CtlProtoType,
    ) -> Self {
        CtlProtoCtx {
            reader,
            writer,
            config,
            protocol_type,
        }
    }

    pub(crate) async fn run(mut self) -> anyhow::Result<()> {
        loop {
            // NOTE allow to change protocol
            match self.protocol_type {
                CtlProtoType::End => break,
                CtlProtoType::Text => {
                    let mut ctx =
                        text::TextCtlCtx::new(&mut self.reader, &mut self.writer, &mut self.config);
                    self.protocol_type = ctx.run().await?;
                }
                CtlProtoType::CapnP => {
                    if let Err(e) = capnp::handle_capnp_connection(self.reader, self.writer) {
                        warn!("upgrade to capnp failed: {e:?}");
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}
