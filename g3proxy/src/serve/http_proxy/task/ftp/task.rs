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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use http::Method;
use log::debug;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use tokio::time::Instant;

use g3_ftp_client::{
    FtpClient, FtpFileFacts, FtpFileListError, FtpFileRetrieveStartError, FtpFileStatError,
    FtpFileStoreStartError, FtpSessionOpenError,
};
use g3_http::server::HttpProxyClientRequest;
use g3_http::{HttpBodyReader, HttpBodyType};
use g3_io_ext::{LimitedCopy, LimitedCopyError, SizedReader};
use g3_types::acl::AclAction;
use g3_types::net::ProxyRequestType;

use super::protocol::{HttpClientReader, HttpClientWriter, HttpProxyRequest};
use super::{
    CommonTaskContext, FtpOverHttpTaskCltWrapperStats, FtpOverHttpTaskStats,
    HttpProxyFtpConnectionProvider, ListWriter,
};
use crate::config::server::ServerConfig;
use crate::log::task::ftp_over_http::TaskLogForFtpOverHttp;
use crate::module::ftp_over_http::{
    BoxFtpRemoteHttpConnection, FtpOverHttpTaskNotes, FtpRequestPath,
};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::module::tcp_connect::TcpConnectError;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

type HttpProxyFtpClient = FtpClient<
    HttpProxyFtpConnectionProvider,
    BoxFtpRemoteHttpConnection,
    TcpConnectError,
    ServerTaskNotes,
>;

pub(crate) struct FtpOverHttpTask<'a> {
    ctx: Arc<CommonTaskContext>,
    req: &'a HttpProxyClientRequest,
    should_close: bool,
    task_notes: ServerTaskNotes,
    ftp_notes: FtpOverHttpTaskNotes,
    task_stats: Arc<FtpOverHttpTaskStats>,
}

impl<'a> FtpOverHttpTask<'a> {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        req: &'a HttpProxyRequest<impl AsyncRead>,
        task_notes: ServerTaskNotes,
    ) -> Self {
        let ftp_notes = FtpOverHttpTaskNotes::new(
            &req.inner,
            &req.upstream,
            ctx.server_config.log_uri_max_chars,
        );
        FtpOverHttpTask {
            ctx: Arc::clone(ctx),
            req: &req.inner,
            should_close: !req.inner.keep_alive(),
            task_notes,
            ftp_notes,
            task_stats: Arc::new(FtpOverHttpTaskStats::default()),
        }
    }

    #[inline]
    pub(crate) fn should_close(&self) -> bool {
        self.should_close
    }

    fn get_log_context(&self) -> TaskLogForFtpOverHttp {
        let http_user_agent = self
            .req
            .end_to_end_headers
            .get(http::header::USER_AGENT)
            .map(|v| v.to_str().unwrap_or("invalid"));
        TaskLogForFtpOverHttp {
            task_notes: &self.task_notes,
            ftp_notes: &self.ftp_notes,
            http_user_agent,
            total_time: self.task_notes.time_elapsed(),
            client_rd_bytes: self.task_stats.http_client.read.get_bytes(),
            client_wr_bytes: self.task_stats.http_client.write.get_bytes(),
            ftp_c_rd_bytes: self.task_stats.ftp_server.control_read.get_bytes(),
            ftp_c_wr_bytes: self.task_stats.ftp_server.control_write.get_bytes(),
            ftp_d_rd_bytes: self.task_stats.ftp_server.transfer_read.get_bytes(),
            ftp_d_wr_bytes: self.task_stats.ftp_server.transfer_write.get_bytes(),
        }
    }

    pub(crate) async fn run<CDR, CDW>(
        &mut self,
        clt_r: &mut HttpClientReader<CDR>,
        clt_w: &mut HttpClientWriter<CDW>,
    ) where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        self.pre_start();
        match self.run_ftp(clt_r, clt_w).await {
            Ok(()) => {
                self.get_log_context()
                    .log(&self.ctx.task_logger, &ServerTaskError::Finished);
            }
            Err(e) => {
                self.get_log_context().log(&self.ctx.task_logger, &e);
            }
        }
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "HttpProxy/FtpOverHttp: new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.task_ftp_over_http.add_task();
        self.ctx.server_stats.task_ftp_over_http.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_total.add_ftp_over_http();
            user_ctx.req_stats().req_alive.add_ftp_over_http();

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_total.add_ftp_over_http();
                site_req_stats.req_alive.add_ftp_over_http();
            }
        }
    }

    fn pre_stop(&mut self) {
        self.ctx.server_stats.task_ftp_over_http.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_alive.del_ftp_over_http();

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_alive.del_ftp_over_http();
            }

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
        }
    }

    fn enable_custom_header_for_local_reply(&self, rsp: &mut HttpProxyClientResponse) {
        self.ctx
            .set_custom_header_for_local_reply(&self.ftp_notes.control_tcp_notes, rsp);
    }

    async fn reply_too_many_requests<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::too_many_requests(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn reply_forbidden_host<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::forbidden(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn reply_banned_protocol<W>(&mut self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::method_not_allowed(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
    }

    async fn handle_server_upstream_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.ctx.server_stats.forbidden.add_dest_denied();
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                // also add to user level forbidden stats
                user_ctx.add_dest_denied();
            }

            self.reply_forbidden_host(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    async fn handle_user_upstream_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.reply_forbidden_host(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    async fn handle_user_protocol_acl_action<W>(
        &mut self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.reply_banned_protocol(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::ProtoBanned,
            ))
        } else {
            Ok(())
        }
    }

    fn setup_clt_limit_and_stats<CDR, CDW>(
        &mut self,
        clt_r: &mut HttpClientReader<CDR>,
        clt_w: &mut HttpClientWriter<CDW>,
    ) where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        let origin_header_size = self.req.origin_header_size() as u64;
        self.task_stats
            .http_client
            .read
            .add_bytes(origin_header_size);

        let mut wrapper_stats =
            FtpOverHttpTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
        let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_io_stats = user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.extra_tags(),
            );
            for s in &user_io_stats {
                s.io.ftp_over_http.add_in_bytes(origin_header_size);
            }
            wrapper_stats.push_user_io_stats(user_io_stats);

            let user = user_ctx.user();
            if user
                .config
                .tcp_sock_speed_limit
                .eq(&self.ctx.server_config.tcp_sock_speed_limit)
            {
                None
            } else {
                let limit_config = user
                    .config
                    .tcp_sock_speed_limit
                    .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
                Some(limit_config)
            }
        } else {
            None
        };

        let (clt_r_stats, clt_w_stats) = wrapper_stats.split();

        clt_w.reset_stats(clt_w_stats);
        clt_r.reset_buffer_stats(clt_r_stats);
        if let Some(limit_config) = &limit_config {
            clt_w.reset_limit(limit_config.shift_millis, limit_config.max_south);
            clt_r.reset_limit(limit_config.shift_millis, limit_config.max_north);
        }
    }

    async fn run_ftp<CDR, CDW>(
        &mut self,
        clt_r: &mut HttpClientReader<CDR>,
        clt_w: &mut HttpClientWriter<CDW>,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Send + Unpin,
    {
        // TODO fetch ftp custom upstream keepalive config
        let mut tcp_client_misc_opts = self.ctx.server_config.tcp_misc_opts;

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_ctx = user_ctx.clone();

            if user_ctx.check_rate_limit().is_err() {
                self.reply_too_many_requests(clt_w).await;
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::RateLimited,
                ));
            }

            match user_ctx.acquire_request_semaphore() {
                Ok(permit) => self.task_notes.user_req_alive_permit = Some(permit),
                Err(_) => {
                    self.reply_too_many_requests(clt_w).await;
                    return Err(ServerTaskError::ForbiddenByRule(
                        ServerTaskForbiddenError::FullyLoaded,
                    ));
                }
            }

            let action = user_ctx.check_proxy_request(ProxyRequestType::FtpOverHttp);
            self.handle_user_protocol_acl_action(action, clt_w).await?;

            let action = user_ctx.check_upstream(self.ftp_notes.upstream());
            self.handle_user_upstream_acl_action(action, clt_w).await?;

            // TODO merge user custom upstream keepalive config
            tcp_client_misc_opts = user_ctx
                .user()
                .config
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // server level dst host/port acl rules
        let action = self.ctx.check_upstream(self.ftp_notes.upstream());
        self.handle_server_upstream_acl_action(action, clt_w)
            .await?;

        // set client side socket options
        g3_socket::tcp::set_raw_opts(self.ctx.tcp_client_socket, &tcp_client_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.setup_clt_limit_and_stats(clt_r, clt_w);

        match self.req.method {
            Method::DELETE => {
                if self.req.body_type().is_some() {
                    return self
                        .reply_bad_request(clt_w, "http body is not allowed in ftp del request")
                        .await;
                }
                let mut ftp_client = self.setup_ftp_client(clt_w, false).await?;
                self.login(&mut ftp_client, clt_w).await?;
                self.delete_path(&mut ftp_client, clt_w).await
            }
            Method::GET => {
                if self.req.body_type().is_some() {
                    return self
                        .reply_bad_request(clt_w, "http body is not allowed in ftp get request")
                        .await;
                }
                let mut ftp_client = self.setup_ftp_client(clt_w, false).await?;
                self.login(&mut ftp_client, clt_w).await?;
                self.list_or_download(&mut ftp_client, clt_w).await
            }
            Method::PUT => {
                if self
                    .req
                    .end_to_end_headers
                    .contains_key(http::header::RANGE)
                {
                    return self
                        .reply_bad_request(clt_w, "Content-Range is not allowed in PUT request")
                        .await;
                }

                if let Some(HttpBodyType::ContentLength(size)) = self.req.body_type() {
                    let mut ftp_client = self.setup_ftp_client(clt_w, false).await?;
                    self.login(&mut ftp_client, clt_w).await?;

                    let body_reader = HttpBodyReader::new(
                        clt_r,
                        HttpBodyType::ContentLength(size),
                        self.ctx.server_config.body_line_max_len,
                    );
                    self.upload(&mut ftp_client, clt_w, body_reader, size).await
                } else {
                    self.reply_bad_request(clt_w, "allow body with fixed content-length only")
                        .await
                }
            }
            _ => self.reply_unimplemented(clt_w).await,
        }
    }

    async fn reply_bad_request<'b, W>(
        &'b mut self,
        clt_w: &'b mut W,
        reason: &'static str,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::bad_request(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
        Err(ServerTaskError::InvalidClientProtocol(reason))
    }

    async fn reply_unimplemented<W>(&mut self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let rsp = HttpProxyClientResponse::unimplemented(self.req.version);
        // no custom header is set
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
        Err(ServerTaskError::UnimplementedProtocol)
    }

    async fn reply_service_unavailable<W>(&mut self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp = HttpProxyClientResponse::service_unavailable(self.req.version);
        self.enable_custom_header_for_local_reply(&mut rsp);
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
        Err(ServerTaskError::UpstreamAppUnavailable)
    }

    async fn reply_bad_gateway<W>(&mut self, clt_w: &mut W, reason: String) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp = HttpProxyClientResponse::bad_gateway(self.req.version);
        self.enable_custom_header_for_local_reply(&mut rsp);
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
        }
        self.should_close = true;
        Err(ServerTaskError::UpstreamNotNegotiated(reason))
    }

    async fn reply_file_unavailable<W>(&mut self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp =
            HttpProxyClientResponse::resource_not_found(self.req.version, self.should_close);
        self.enable_custom_header_for_local_reply(&mut rsp);
        match rsp.reply_err_to_request(clt_w).await {
            Ok(_) => {
                self.ftp_notes.rsp_status = rsp.status();
                Err(ServerTaskError::Finished)
            }
            Err(e) => {
                self.should_close = true;
                Err(ServerTaskError::ClientTcpWriteFailed(e))
            }
        }
    }

    async fn reply_range_not_satisfiable<W>(
        &mut self,
        clt_w: &mut W,
        valid_start_size: Option<u64>,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut rsp = HttpProxyClientResponse::range_not_satisfiable(
            self.req.version,
            self.should_close,
            valid_start_size,
        );
        self.enable_custom_header_for_local_reply(&mut rsp);
        match rsp.reply_err_to_request(clt_w).await {
            Ok(_) => {
                self.ftp_notes.rsp_status = rsp.status();
                if valid_start_size.is_some() {
                    Err(ServerTaskError::Finished)
                } else {
                    Err(ServerTaskError::InvalidClientProtocol("invalid range"))
                }
            }
            Err(e) => {
                self.should_close = true;
                Err(ServerTaskError::ClientTcpWriteFailed(e))
            }
        }
    }

    async fn reply_unauthorized<W>(&mut self, clt_w: &mut W, reason: String) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        // create the realm string as apache2 mod_proxy_ftp
        let realm = if let Some(user) = self.ftp_notes.username() {
            format!("ftp://{}@{}", user.to_encoded(), self.ftp_notes.upstream())
        } else {
            format!("ftp://{}", self.ftp_notes.upstream())
        };
        let mut rsp = HttpProxyClientResponse::need_login(
            self.req.version,
            self.should_close || self.req.body_type().is_some(),
            &realm,
        );
        self.enable_custom_header_for_local_reply(&mut rsp);
        if rsp.reply_err_to_request(clt_w).await.is_ok() {
            self.ftp_notes.rsp_status = rsp.status();
            self.should_close = rsp.should_close();
        } else {
            self.should_close = true;
        }
        Err(ServerTaskError::UpstreamNotNegotiated(reason))
    }

    async fn setup_ftp_client<'b, W>(
        &'b mut self,
        clt_w: &'b mut W,
        body_pending: bool,
    ) -> ServerTaskResult<HttpProxyFtpClient>
    where
        W: AsyncWrite + Unpin,
    {
        let escaper_connect_context = self
            .ctx
            .escaper
            .new_ftp_connect_context(
                Arc::clone(&self.ctx.escaper),
                &self.task_notes,
                self.ftp_notes.upstream(),
            )
            .await;
        let ftp_connection_provider =
            HttpProxyFtpConnectionProvider::new(&self.task_stats, escaper_connect_context);

        self.task_notes.stage = ServerTaskStage::Connecting;
        match FtpClient::connect_to(
            self.ftp_notes.upstream().clone(),
            ftp_connection_provider,
            &self.task_notes,
            &self.ctx.server_config.ftp_client_config,
        )
        .await
        {
            Ok(client) => {
                self.task_notes.stage = ServerTaskStage::Connected;
                client
                    .connection_provider()
                    .connect_context()
                    .fetch_control_tcp_notes(&mut self.ftp_notes.control_tcp_notes);
                Ok(client)
            }
            Err((e, ftp_connection_provider)) => {
                ftp_connection_provider
                    .connect_context()
                    .fetch_control_tcp_notes(&mut self.ftp_notes.control_tcp_notes);
                let mut rsp = HttpProxyClientResponse::from_ftp_connect_error(
                    &e,
                    self.req.version,
                    self.should_close || body_pending,
                );
                self.enable_custom_header_for_local_reply(&mut rsp);
                if rsp.reply_err_to_request(clt_w).await.is_ok() {
                    self.ftp_notes.rsp_status = rsp.status();
                    self.should_close = rsp.should_close();
                } else {
                    self.should_close = true;
                }
                Err(e.into())
            }
        }
    }

    async fn login<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        match ftp_client
            .new_user_session(self.ftp_notes.username(), self.ftp_notes.password())
            .await
        {
            Ok(_) => {
                self.task_notes.stage = ServerTaskStage::LoggedIn;
                Ok(())
            }
            Err(e) => match e {
                FtpSessionOpenError::RawCommandError(_) => {
                    self.reply_bad_gateway(clt_w, format!("user login failed: {e:?}"))
                        .await
                }
                FtpSessionOpenError::ServiceNotAvailable => {
                    self.reply_service_unavailable(clt_w).await
                }
                FtpSessionOpenError::NotLoggedIn | FtpSessionOpenError::AccountIsNeeded => {
                    self.reply_unauthorized(clt_w, format!("user login failed: {e:?}"))
                        .await
                }
            },
        }
    }

    async fn delete_path<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Send + Unpin,
    {
        let r = match &self.ftp_notes.ftp_path {
            FtpRequestPath::DefaultDir(_) => {
                return self
                    .reply_bad_request(clt_w, "path is required in ftp delete request")
                    .await;
            }
            FtpRequestPath::ListOnly(dir) => ftp_client.remove_dir(dir).await,
            FtpRequestPath::FileOnly(file) => ftp_client.delete_file(file).await,
            FtpRequestPath::AutoDetect(path) => match ftp_client.fetch_file_facts(path).await {
                Ok(facts) => {
                    if facts.maybe_file() && facts.size().is_some() {
                        ftp_client.delete_file(path).await
                    } else {
                        ftp_client.remove_dir(path).await
                    }
                }
                Err(e) => Err(e),
            },
        };

        match r {
            Ok(_) => Ok(()),
            Err(FtpFileStatError::FileUnavailable) => self.reply_file_unavailable(clt_w).await,
            Err(FtpFileStatError::ServiceNotAvailable) => {
                self.reply_service_unavailable(clt_w).await
            }
            Err(e) => {
                self.reply_bad_gateway(clt_w, format!("ftp delete failed: {e:?}"))
                    .await
            }
        }
    }

    async fn list_or_download<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Send + Unpin,
    {
        if self.ftp_notes.ftp_path.detect_fact() {
            match ftp_client
                .fetch_file_facts(self.ftp_notes.ftp_path.as_str())
                .await
            {
                Ok(facts) => {
                    if facts.maybe_file() && facts.size().is_some() {
                        return self.download_file(ftp_client, &facts, clt_w).await;
                    }
                }
                Err(FtpFileStatError::FileUnavailable) => {
                    return self.reply_file_unavailable(clt_w).await;
                }
                Err(FtpFileStatError::ServiceNotAvailable) => {
                    return self.reply_service_unavailable(clt_w).await;
                }
                Err(e) => {
                    return self
                        .reply_bad_gateway(clt_w, format!("ftp stat failed: {e:?}"))
                        .await;
                }
            }
        }
        self.list_entry(ftp_client, clt_w).await
    }

    async fn list_entry<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Send + Unpin,
    {
        match ftp_client
            .list_directory_detailed_start(self.ftp_notes.ftp_path.as_str(), &self.task_notes)
            .await
        {
            Ok(data_stream) => {
                ftp_client
                    .connection_provider()
                    .connect_context()
                    .fetch_transfer_tcp_notes(&mut self.ftp_notes.transfer_tcp_notes);

                self.task_notes.stage = ServerTaskStage::Replying;
                let (mut rsp, chunked) = HttpProxyClientResponse::auto_chunked_ok(
                    self.req.version,
                    self.should_close,
                    &mime::TEXT_PLAIN,
                );
                self.enable_custom_header_for_local_reply(&mut rsp);
                rsp.reply_ok_header(clt_w).await.map_err(|e| {
                    self.should_close = true;
                    ServerTaskError::ClientTcpWriteFailed(e)
                })?;
                self.ftp_notes.rsp_status = rsp.status();

                self.task_notes.mark_relaying();
                let ret = if chunked {
                    let mut receiver = super::ChunkedListWriter::new(
                        clt_w,
                        self.ctx.server_config.tcp_copy.buffer_size(),
                    );
                    self.receive_list_data(ftp_client, data_stream, &mut receiver)
                        .await
                } else {
                    let mut receiver = super::EndingListWriter::new(
                        clt_w,
                        self.ctx.server_config.tcp_copy.buffer_size(),
                    );
                    self.receive_list_data(ftp_client, data_stream, &mut receiver)
                        .await
                };
                if ret.is_err() {
                    // close the client side connection as we have failed to write body
                    self.should_close = true;
                }
                ret
            }
            Err(FtpFileRetrieveStartError::ServiceNotAvailable) => {
                self.reply_service_unavailable(clt_w).await
            }
            Err(FtpFileRetrieveStartError::FileUnavailable) => {
                self.reply_file_unavailable(clt_w).await
            }
            Err(e) => {
                self.reply_bad_gateway(clt_w, format!("ftp list start failed: {e:?}"))
                    .await
            }
        }
    }

    async fn receive_list_data<R>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        data_stream: BoxFtpRemoteHttpConnection,
        receiver: &mut R,
    ) -> ServerTaskResult<()>
    where
        R: ListWriter,
    {
        match ftp_client
            .list_directory_detailed_receive(data_stream, receiver)
            .await
        {
            Ok(()) => {
                receiver
                    .flush_buf()
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;

                self.task_notes.stage = ServerTaskStage::Finished;
                Ok(())
            }
            Err(e) => {
                if let FtpFileListError::LocalIoCallbackFailed = e {
                    if let Some(io_err) = receiver.take_io_error() {
                        return Err(ServerTaskError::ClientTcpWriteFailed(io_err));
                    }
                }

                receiver
                    .flush_buf()
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;

                Err(ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))
            }
        }
    }

    fn get_download_range(&self) -> (Option<u64>, Option<u64>) {
        let mut start: Option<u64> = None;
        let mut end: Option<u64> = None;
        let headers = self.req.end_to_end_headers.get_all(http::header::RANGE);
        for v in headers {
            if start.is_some() || end.is_some() {
                // we don't support multiple ranges
                return (None, None);
            }

            let value = unsafe { std::str::from_utf8_unchecked(v.as_bytes()) };
            let value = value.trim();
            if !value.starts_with("bytes=") {
                return (None, None);
            }

            let ranges = &value[6..];
            if ranges.is_empty() {
                return (None, None);
            }
            for r in ranges.split(',') {
                let range = r.trim();
                match memchr::memrchr(b'-', range.as_bytes()) {
                    Some(p) => {
                        start = if p > 0 {
                            u64::from_str(&range[0..p]).map(Some).unwrap_or(None)
                        } else {
                            None
                        };

                        end = if p + 1 < range.len() {
                            u64::from_str(&range[p + 1..]).map(Some).unwrap_or(None)
                        } else {
                            None
                        };
                    }
                    None => return (None, None),
                }
            }
        }
        (start, end)
    }

    async fn download_file<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        file_facts: &FtpFileFacts,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        if let (Some(start), end) = self.get_download_range() {
            self.download_file_from_position(ftp_client, file_facts, start, end, clt_w)
                .await
        } else {
            self.download_full_file(ftp_client, file_facts, clt_w).await
        }
    }

    async fn download_full_file<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        file_facts: &FtpFileFacts,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        match ftp_client
            .retrieve_file_start(self.ftp_notes.ftp_path.as_str(), None, &self.task_notes)
            .await
        {
            Ok((data_stream, file_transfer_size)) => {
                ftp_client
                    .connection_provider()
                    .connect_context()
                    .fetch_transfer_tcp_notes(&mut self.ftp_notes.transfer_tcp_notes);

                self.task_notes.stage = ServerTaskStage::Replying;
                let mime = file_facts
                    .media_type()
                    .unwrap_or(&mime::APPLICATION_OCTET_STREAM);
                if let Some(size) = file_transfer_size {
                    let mut rsp = HttpProxyClientResponse::sized_ok(
                        self.req.version,
                        self.should_close,
                        size,
                        mime,
                    );
                    self.enable_custom_header_for_local_reply(&mut rsp);
                    rsp.reply_ok_header(clt_w).await.map_err(|e| {
                        self.should_close = true;
                        ServerTaskError::ClientTcpWriteFailed(e)
                    })?;
                    self.ftp_notes.rsp_status = rsp.status();

                    self.task_notes.mark_relaying();

                    match self
                        .receive_file_data(
                            ftp_client,
                            SizedReader::new(data_stream, size),
                            false,
                            clt_w,
                        )
                        .await
                    {
                        Ok(copied_size) => {
                            if copied_size != size {
                                self.should_close = true;
                                Err(ServerTaskError::UpstreamAppError(anyhow!(
                                    "copied {} bytes different than expected {}",
                                    copied_size,
                                    size
                                )))
                            } else {
                                Ok(())
                            }
                        }
                        Err(e) => {
                            // close the client side connection as we have failed to write body
                            self.should_close = true;
                            Err(e)
                        }
                    }
                } else {
                    let mut rsp = HttpProxyClientResponse::ending_ok(
                        self.req.version,
                        self.should_close,
                        mime,
                    );
                    self.enable_custom_header_for_local_reply(&mut rsp);
                    self.should_close = true; // always close the connection
                    rsp.reply_ok_header(clt_w)
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    self.ftp_notes.rsp_status = rsp.status();

                    self.task_notes.mark_relaying();

                    match self
                        .receive_file_data(ftp_client, data_stream, false, clt_w)
                        .await
                    {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            // close the client side connection as we have failed to write body
                            self.should_close = true;
                            Err(e)
                        }
                    }
                }
            }
            Err(FtpFileRetrieveStartError::ServiceNotAvailable) => {
                self.reply_service_unavailable(clt_w).await
            }
            Err(FtpFileRetrieveStartError::FileUnavailable) => {
                self.reply_file_unavailable(clt_w).await
            }
            Err(e) => {
                self.reply_bad_gateway(clt_w, format!("ftp retrieve start failed: {e:?}"))
                    .await
            }
        }
    }

    async fn download_file_from_position<W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        file_facts: &FtpFileFacts,
        start_size: u64,
        end_size: Option<u64>,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        match ftp_client
            .retrieve_file_start(
                self.ftp_notes.ftp_path.as_str(),
                Some(start_size),
                &self.task_notes,
            )
            .await
        {
            Ok((data_stream, file_transfer_size)) => {
                ftp_client
                    .connection_provider()
                    .connect_context()
                    .fetch_transfer_tcp_notes(&mut self.ftp_notes.transfer_tcp_notes);

                self.task_notes.stage = ServerTaskStage::Replying;
                let mime = file_facts
                    .media_type()
                    .unwrap_or(&mime::APPLICATION_OCTET_STREAM);
                if let Some(file_size) = file_transfer_size {
                    let end_size = end_size.unwrap_or(file_size - 1);
                    if end_size < start_size {
                        return self.reply_range_not_satisfiable(clt_w, None).await;
                    }
                    let file_copy_size = end_size - start_size + 1;

                    let mut rsp = HttpProxyClientResponse::sized_partial_content(
                        self.req.version,
                        self.should_close,
                        start_size,
                        end_size,
                        file_size,
                        mime,
                    );
                    self.enable_custom_header_for_local_reply(&mut rsp);
                    rsp.reply_ok_header(clt_w).await.map_err(|e| {
                        self.should_close = true;
                        ServerTaskError::ClientTcpWriteFailed(e)
                    })?;
                    self.ftp_notes.rsp_status = rsp.status();

                    self.task_notes.mark_relaying();

                    match self
                        .receive_file_data(
                            ftp_client,
                            SizedReader::new(data_stream, file_copy_size),
                            file_size != end_size + 1,
                            clt_w,
                        )
                        .await
                    {
                        Ok(copied_size) => {
                            if copied_size != file_copy_size {
                                self.should_close = true;
                                Err(ServerTaskError::UpstreamAppError(anyhow!(
                                    "copied {} bytes different than expected {}",
                                    copied_size,
                                    file_copy_size
                                )))
                            } else {
                                Ok(())
                            }
                        }
                        Err(e) => {
                            // close the client side connection as we have failed to write body
                            self.should_close = true;
                            Err(e)
                        }
                    }
                } else {
                    if let Some(end_size) = end_size {
                        if end_size < start_size {
                            return self.reply_range_not_satisfiable(clt_w, None).await;
                        }
                    }
                    self.reply_range_not_satisfiable(clt_w, Some(start_size))
                        .await
                }
            }
            Err(FtpFileRetrieveStartError::ServiceNotAvailable) => {
                self.reply_service_unavailable(clt_w).await
            }
            Err(FtpFileRetrieveStartError::FileUnavailable) => {
                self.reply_file_unavailable(clt_w).await
            }
            Err(e) => {
                self.reply_bad_gateway(clt_w, format!("ftp retrieve start failed: {e:?}"))
                    .await
            }
        }
    }

    async fn receive_file_data<'b, S, W>(
        &'b mut self,
        ftp_client: &'b mut HttpProxyFtpClient,
        mut data_stream: S,
        should_abort: bool,
        clt_w: &'b mut W,
    ) -> ServerTaskResult<u64>
    where
        S: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut data_copy =
            LimitedCopy::new(&mut data_stream, clt_w, &self.ctx.server_config.tcp_copy);

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut data_copy => {
                    if should_abort {
                        ftp_client.abort_transfer()
                            .await
                            .map_err(|e| ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))?;
                    } else {
                        ftp_client.wait_retrieve_end_reply()
                            .await
                            .map_err(|e| ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))?;
                    }
                    r.map_err(|e| match e {
                        LimitedCopyError::ReadFailed(e) => ServerTaskError::UpstreamReadFailed(e),
                        LimitedCopyError::WriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
                    })?;

                    self.task_notes.stage = ServerTaskStage::Finished;
                    return Ok(data_copy.copied_size());
                }
                r = ftp_client.wait_control_read_ready() => {
                    if let Err(e) = r {
                        return Err(ServerTaskError::UpstreamAppError(anyhow::Error::new(e)));
                    }
                    ftp_client.wait_retrieve_end_reply()
                        .await
                        .map_err(|e| ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))?;

                    let wait_timeout = ftp_client.transfer_end_wait_timeout();
                    return match tokio::time::timeout(wait_timeout, &mut data_copy).await {
                        Ok(Ok(_)) => {
                            self.task_notes.stage = ServerTaskStage::Finished;
                            Ok(data_copy.copied_size())
                        }
                        Ok(Err(LimitedCopyError::ReadFailed(e))) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Ok(Err(LimitedCopyError::WriteFailed(e))) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                        Err(_) => Err(ServerTaskError::UpstreamAppTimeout("timeout to wait transfer end")),
                    };
                }
                _ = idle_interval.tick() => {
                    if data_copy.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.ctx.server_config.task_idle_max_count
                        };

                        if quit {
                            return if data_copy.no_cached_data() {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while reading data"))
                            } else {
                                Err(ServerTaskError::ClientAppTimeout("idle while writing data"))
                            };
                        }
                    } else {
                        idle_count = 0;

                        data_copy.reset_active();
                    }

                    if let Some(user_ctx) = self.task_notes.user_ctx() {
                        if user_ctx.user().is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    async fn check_and_send_continue<W>(&mut self, clt_w: &mut W) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        if matches!(
            self.req.version,
            http::Version::HTTP_09 | http::Version::HTTP_10
        ) {
            return Ok(());
        }

        if let Some(v) = self.req.end_to_end_headers.get(http::header::EXPECT) {
            if let Ok(s) = std::str::from_utf8(v.as_bytes()) {
                if s.to_lowercase().eq("100-continue") {
                    if let Err(e) =
                        HttpProxyClientResponse::reply_continue(self.req.version, clt_w).await
                    {
                        self.should_close = true;
                        return Err(ServerTaskError::ClientTcpWriteFailed(e));
                    }
                }
            }
        }

        Ok(())
    }

    async fn upload<R, W>(
        &mut self,
        ftp_client: &mut HttpProxyFtpClient,
        clt_w: &mut W,
        body_reader: HttpBodyReader<'_, R>,
        expected_size: u64,
    ) -> ServerTaskResult<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        match ftp_client
            .store_file_start(self.ftp_notes.ftp_path.as_str(), &self.task_notes)
            .await
        {
            Ok(data_stream) => {
                ftp_client
                    .connection_provider()
                    .connect_context()
                    .fetch_transfer_tcp_notes(&mut self.ftp_notes.transfer_tcp_notes);

                self.check_and_send_continue(clt_w).await?;

                self.task_notes.mark_relaying();

                match self
                    .send_file_data(ftp_client, data_stream, body_reader)
                    .await
                {
                    Ok(copied_size) => {
                        if copied_size != expected_size {
                            self.reply_bad_gateway(
                                clt_w,
                                format!(
                                    "uploaded {copied_size} bytes different than expected {expected_size}"
                                ),
                            )
                            .await
                        } else {
                            let mut rsp =
                                HttpProxyClientResponse::ok(self.req.version, self.should_close);
                            self.enable_custom_header_for_local_reply(&mut rsp);
                            match rsp.reply_ok_header(clt_w).await {
                                Ok(_) => {
                                    self.ftp_notes.rsp_status = rsp.status();
                                    self.task_notes.stage = ServerTaskStage::Finished;
                                    Ok(())
                                }
                                Err(e) => {
                                    self.should_close = true;
                                    Err(ServerTaskError::ClientTcpWriteFailed(e))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.should_close = true;
                        if let Some(mut rsp) =
                            HttpProxyClientResponse::from_task_err(&e, self.req.version, true)
                        {
                            self.enable_custom_header_for_local_reply(&mut rsp);
                            rsp.reply_err_to_request(clt_w)
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            self.ftp_notes.rsp_status = rsp.status();
                        }
                        Err(e)
                    }
                }
            }
            Err(FtpFileStoreStartError::ServiceNotAvailable) => {
                self.reply_service_unavailable(clt_w).await
            }
            Err(FtpFileStoreStartError::FileUnavailable) => {
                self.reply_file_unavailable(clt_w).await
            }
            Err(e) => {
                self.reply_bad_gateway(clt_w, format!("ftp retrieve start failed: {e:?}"))
                    .await
            }
        }
    }

    async fn send_file_data<'b, S, R>(
        &'b mut self,
        ftp_client: &'b mut HttpProxyFtpClient,
        mut data_stream: S,
        mut body_reader: HttpBodyReader<'_, R>,
    ) -> ServerTaskResult<u64>
    where
        S: AsyncWrite + Unpin,
        R: AsyncBufRead + Unpin,
    {
        let mut data_copy = LimitedCopy::new(
            &mut body_reader,
            &mut data_stream,
            &self.ctx.server_config.tcp_copy,
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut data_copy => {
                    let copied_size = data_copy.copied_size();
                    drop(data_stream);

                    ftp_client.wait_store_end_reply()
                        .await
                        .map_err(|e| ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))?;
                    r.map_err(|e| match e {
                        LimitedCopyError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
                        LimitedCopyError::WriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
                    })?;
                    return Ok(copied_size);
                }
                r = ftp_client.wait_control_read_ready() => {
                    if let Err(e) = r {
                        return Err(ServerTaskError::UpstreamAppError(anyhow::Error::new(e)));
                    }
                    ftp_client.wait_store_end_reply()
                        .await
                        .map_err(|e| ServerTaskError::UpstreamAppError(anyhow::Error::new(e)))?;

                    return Err(ServerTaskError::UpstreamAppError(
                        anyhow!("unexpected server end reply after {} bytes sent)", data_copy.copied_size())
                    ));
                }
                _ = idle_interval.tick() => {
                    if data_copy.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.ctx.server_config.task_idle_max_count
                        };

                        if quit {
                            return if data_copy.no_cached_data() {
                                Err(ServerTaskError::ClientAppTimeout("idle while reading data"))
                            } else {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while sending data"))
                            };
                        }
                    } else {
                        idle_count = 0;

                        data_copy.reset_active();
                    }

                    if let Some(user_ctx) = self.task_notes.user_ctx() {
                        if user_ctx.user().is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }
}
