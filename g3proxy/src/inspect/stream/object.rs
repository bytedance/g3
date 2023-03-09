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

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt};

use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::{FlexBufReader, OnceBufReader};
use g3_types::net::UpstreamAddr;

use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::log::inspect::stream::StreamInspectLog;
use crate::log::inspect::InspectSource;
use crate::serve::{ServerTaskError, ServerTaskResult};

enum InitialDataSource {
    Client,
    Server,
}

struct StreamInspectIo {
    clt_r: BoxAsyncRead,
    clt_w: BoxAsyncWrite,
    ups_r: BoxAsyncRead,
    ups_w: BoxAsyncWrite,
}

pub(crate) struct StreamInspectObject<SC: ServerConfig> {
    io: Option<StreamInspectIo>,
    pub(super) ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
}

impl<SC> StreamInspectObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        StreamInspectObject {
            io: None,
            ctx,
            upstream,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = StreamInspectIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    pub(super) async fn transit_unknown(mut self) -> ServerTaskResult<()> {
        let StreamInspectIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        self.ctx.transit_unknown(clt_r, clt_w, ups_r, ups_w).await
    }

    pub(super) async fn transit_with_inspection(
        mut self,
        inspector: &mut ProtocolInspector,
    ) -> ServerTaskResult<StreamInspection<SC>> {
        let StreamInspectIo {
            mut clt_r,
            clt_w,
            mut ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let inspect_buffer_size = self.ctx.protocol_inspection().data0_buffer_size();
        let mut clt_r_buf = BytesMut::with_capacity(inspect_buffer_size);
        let mut ups_r_buf = BytesMut::with_capacity(inspect_buffer_size);

        let data_source = match tokio::time::timeout(
            self.ctx.protocol_inspection().data0_wait_timeout(),
            self.wait_initial_data(&mut clt_r, &mut clt_r_buf, &mut ups_r, &mut ups_r_buf),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Ok(StreamInspection::StreamUnknown(self)),
        };

        let protocol = match tokio::time::timeout(
            self.ctx.protocol_inspection().data0_read_timeout(),
            self.inspect_initial_data(
                data_source,
                inspector,
                &mut clt_r,
                &mut clt_r_buf,
                &mut ups_r,
                &mut ups_r_buf,
            ),
        )
        .await
        {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Ok(StreamInspection::StreamUnknown(self)),
        };

        self.ctx.increase_inspection_depth();
        StreamInspectLog::new(&self.ctx).log(InspectSource::StreamInspection, protocol);
        match protocol {
            Protocol::Unknown => {
                self.ctx
                    .transit_unknown(
                        OnceBufReader::new(clt_r, clt_r_buf),
                        clt_w,
                        OnceBufReader::new(ups_r, ups_r_buf),
                        ups_w,
                    )
                    .await?;
                return Ok(StreamInspection::End);
            }
            Protocol::TlsModern => {
                if let Some(tls_interception) = self.ctx.tls_interception() {
                    let mut tls_obj = crate::inspect::tls::TlsInterceptObject::new(
                        self.ctx,
                        self.upstream,
                        tls_interception,
                    );
                    tls_obj.set_io(OnceBufReader::new(clt_r, clt_r_buf), clt_w, ups_r, ups_w);
                    return Ok(StreamInspection::TlsModern(tls_obj));
                }
            }
            Protocol::Http1 => {
                let mut h1_obj = crate::inspect::http::H1InterceptObject::new(self.ctx);
                h1_obj.set_io(
                    FlexBufReader::with_bytes(clt_r_buf, clt_r),
                    clt_w,
                    ups_r,
                    ups_w,
                );
                return Ok(StreamInspection::H1(h1_obj));
            }
            Protocol::Http2 => {
                let mut h2_obj = crate::inspect::http::H2InterceptObject::new(self.ctx);
                h2_obj.set_io(OnceBufReader::new(clt_r, clt_r_buf), clt_w, ups_r, ups_w);
                return Ok(StreamInspection::H2(h2_obj));
            }
            _ => {}
        }

        self.ctx
            .transit_transparent(
                OnceBufReader::new(clt_r, clt_r_buf),
                clt_w,
                OnceBufReader::new(ups_r, ups_r_buf),
                ups_w,
            )
            .await?;
        Ok(StreamInspection::End)
    }

    async fn wait_initial_data(
        &mut self,
        clt_r: &mut BoxAsyncRead,
        clt_r_buf: &mut BytesMut,
        ups_r: &mut BoxAsyncRead,
        ups_r_buf: &mut BytesMut,
    ) -> ServerTaskResult<InitialDataSource> {
        tokio::select! {
            biased;

            r = clt_r.read_buf(clt_r_buf) => {
                match r {
                    Ok(_) => Ok(InitialDataSource::Client),
                    Err(e) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                }
            }
            r = ups_r.read_buf(ups_r_buf) => {
                match r {
                    Ok(_) => Ok(InitialDataSource::Server),
                    Err(e) => Err(ServerTaskError::UpstreamReadFailed(e)),
                }
            }
        }
    }

    async fn inspect_initial_data(
        &mut self,
        source: InitialDataSource,
        inspector: &mut ProtocolInspector,
        clt_r: &mut BoxAsyncRead,
        clt_r_buf: &mut BytesMut,
        ups_r: &mut BoxAsyncRead,
        ups_r_buf: &mut BytesMut,
    ) -> ServerTaskResult<Protocol> {
        match source {
            InitialDataSource::Client => {
                self.inspect_client_data(inspector, clt_r_buf, clt_r).await
            }
            InitialDataSource::Server => {
                self.inspect_server_data(inspector, ups_r_buf, ups_r).await
            }
        }
    }

    async fn inspect_client_data<CR>(
        &mut self,
        inspector: &mut ProtocolInspector,
        clt_r_buf: &mut BytesMut,
        clt_r: &mut CR,
    ) -> Result<Protocol, ServerTaskError>
    where
        CR: AsyncRead + Unpin,
    {
        loop {
            match inspector.check_client_initial_data(
                self.ctx.protocol_inspection(),
                self.upstream.port(),
                clt_r_buf.chunk(),
            ) {
                Ok(p) => return Ok(p),
                Err(_) => {
                    if clt_r_buf.remaining() == 0 {
                        return Ok(Protocol::Unknown);
                    }
                    match clt_r.read_buf(clt_r_buf).await {
                        Ok(0) => return Err(ServerTaskError::ClosedByClient),
                        Ok(_) => {}
                        Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
                    }
                }
            }
        }
    }

    async fn inspect_server_data<UR>(
        &mut self,
        inspector: &mut ProtocolInspector,
        ups_r_buf: &mut BytesMut,
        ups_r: &mut UR,
    ) -> Result<Protocol, ServerTaskError>
    where
        UR: AsyncRead + Unpin,
    {
        loop {
            match inspector.check_server_initial_data(
                self.ctx.protocol_inspection(),
                self.upstream.port(),
                ups_r_buf.chunk(),
            ) {
                Ok(p) => return Ok(p),
                Err(_) => {
                    if ups_r_buf.remaining() == 0 {
                        return Ok(Protocol::Unknown);
                    }
                    match ups_r.read_buf(ups_r_buf).await {
                        Ok(0) => return Err(ServerTaskError::ClosedByUpstream),
                        Ok(_) => {}
                        Err(e) => return Err(ServerTaskError::UpstreamReadFailed(e)),
                    }
                }
            }
        }
    }
}
