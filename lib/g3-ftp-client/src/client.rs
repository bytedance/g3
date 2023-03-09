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

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_types::auth::{Password, Username};
use g3_types::net::UpstreamAddr;

use crate::control::FtpCommand;
use crate::error::{
    FtpAuthStatus, FtpCommandError, FtpConnectError, FtpFileListError, FtpFilePreTransferStatus,
    FtpFileRetrieveError, FtpFileRetrieveStartError, FtpFileStatError, FtpFileStoreError,
    FtpFileStoreStartError, FtpRawResponseError, FtpSessionOpenError, FtpTransferSetupError,
};
use crate::transfer::{FtpLineDataReceiver, FtpLineDataTransfer, FtpTransferType};
use crate::{
    log_msg, FtpClientConfig, FtpConnectionProvider, FtpControlChannel, FtpFileFacts,
    FtpServerFeature,
};

pub struct FtpClient<CP, S, E, UD>
where
    CP: FtpConnectionProvider<S, E, UD>,
    S: AsyncRead + AsyncWrite,
    E: std::error::Error,
{
    server: UpstreamAddr,
    conn_provider: CP,
    config: Arc<FtpClientConfig>,
    control: FtpControlChannel<S>,
    server_feature: FtpServerFeature,
    transfer_type: FtpTransferType,
    _phantom_e: PhantomData<E>,
    _phantom_ud: PhantomData<UD>,
}

impl<CP, S, E, UD> FtpClient<CP, S, E, UD>
where
    CP: FtpConnectionProvider<S, E, UD>,
    S: AsyncRead + AsyncWrite + Unpin,
    E: std::error::Error,
{
    #[inline]
    pub fn connection_provider(&self) -> &CP {
        &self.conn_provider
    }

    pub async fn connect_to(
        server: UpstreamAddr,
        mut conn_provider: CP,
        user_data: &UD,
        config: &Arc<FtpClientConfig>,
    ) -> Result<Self, (FtpConnectError<E>, CP)> {
        let control_stream = match tokio::time::timeout(
            config.connect_timeout,
            conn_provider.new_control_connection(&server, user_data),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                return Err((FtpConnectError::ConnectIoError(e), conn_provider));
            }
            Err(_) => {
                return Err((FtpConnectError::ConnectTimedOut, conn_provider));
            }
        };

        let mut control = FtpControlChannel::new(control_stream, config.control);
        match tokio::time::timeout(config.greeting_timeout, control.wait_greetings()).await {
            Ok(Ok(_)) => {}
            Ok(Err(FtpCommandError::ServiceNotAvailable)) => {
                return Err((FtpConnectError::ServiceNotAvailable, conn_provider));
            }
            Ok(Err(e)) => {
                return Err((FtpConnectError::GreetingFailed(e), conn_provider));
            }
            Err(_) => {
                return Err((FtpConnectError::GreetingTimedOut, conn_provider));
            }
        }

        let server_feature = match control.check_server_feature().await {
            Ok(feature) => feature,
            Err(FtpCommandError::ServiceNotAvailable) => {
                return Err((FtpConnectError::ServiceNotAvailable, conn_provider));
            }
            Err(e) => {
                return Err((FtpConnectError::NegotiationFailed(e), conn_provider));
            }
        };
        if server_feature.support_utf8_path() {
            // ignore the server reply
            let _ = control.set_use_utf8().await;
        }

        Ok(FtpClient {
            server,
            conn_provider,
            config: Arc::clone(config),
            control,
            server_feature,
            transfer_type: FtpTransferType::Ascii,
            _phantom_e: Default::default(),
            _phantom_ud: Default::default(),
        })
    }

    pub async fn new_user_session(
        &mut self,
        name: Option<&Username>,
        pass: Option<&Password>,
    ) -> Result<(), FtpSessionOpenError> {
        match self.control.send_username(name).await? {
            FtpAuthStatus::NotLoggedIn => return Err(FtpSessionOpenError::NotLoggedIn),
            FtpAuthStatus::LoggedIn => return Ok(()),
            FtpAuthStatus::NeedPassword => {}
            FtpAuthStatus::NeedAccount => return Err(FtpSessionOpenError::AccountIsNeeded),
        }

        match self.control.send_password(pass).await? {
            FtpAuthStatus::NotLoggedIn | FtpAuthStatus::NeedPassword => {
                Err(FtpSessionOpenError::NotLoggedIn)
            }
            FtpAuthStatus::LoggedIn => Ok(()),
            FtpAuthStatus::NeedAccount => Err(FtpSessionOpenError::AccountIsNeeded),
        }
    }

    pub async fn quit_and_close(&mut self) -> Result<(), FtpCommandError> {
        self.control.send_quit().await
    }

    pub async fn delete_file(&mut self, path: &str) -> Result<(), FtpFileStatError> {
        self.control.delete_file(path).await
    }

    pub async fn remove_dir(&mut self, path: &str) -> Result<(), FtpFileStatError> {
        self.control.remove_dir(path).await
    }

    pub async fn fetch_file_facts(&mut self, path: &str) -> Result<FtpFileFacts, FtpFileStatError> {
        let mut ff = if self.server_feature.support_machine_list() {
            match self.control.request_mlst(path).await {
                Ok(Some(ff)) => ff,
                Ok(None) => return Err(FtpFileStatError::FileUnavailable),
                Err(FtpCommandError::CommandNotImplemented(_)) => FtpFileFacts::new(path),
                Err(e) => return Err(e.into()),
            }
        } else {
            FtpFileFacts::new(path)
        };

        if ff.size().is_none() && self.server_feature.support_file_size() && ff.maybe_file() {
            match self.control.request_size(path).await {
                Ok(Some(size)) => ff.set_size(size),
                Ok(None) => {}
                Err(FtpCommandError::CommandNotImplemented(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }

        if ff.mtime().is_none() && self.server_feature.support_file_mtime() {
            match self.control.request_mtime(path).await {
                Ok(Some(dt)) => ff.set_mtime(dt),
                Ok(None) => {}
                Err(FtpCommandError::CommandNotImplemented(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }

        Ok(ff)
    }

    async fn use_ascii_transfer(&mut self) -> Result<(), FtpCommandError> {
        if !matches!(self.transfer_type, FtpTransferType::Ascii) {
            self.control
                .request_transfer_type(FtpTransferType::Ascii)
                .await?;
            self.transfer_type = FtpTransferType::Ascii;
        }
        Ok(())
    }

    async fn use_binary_transfer(&mut self) -> Result<(), FtpCommandError> {
        if !matches!(self.transfer_type, FtpTransferType::Image) {
            self.control
                .request_transfer_type(FtpTransferType::Image)
                .await?;
            self.transfer_type = FtpTransferType::Image;
        }
        Ok(())
    }

    async fn new_epsv_data_transfer<'a>(
        &'a mut self,
        user_data: &'a UD,
    ) -> Result<S, FtpTransferSetupError> {
        let port = self.control.request_epsv_port().await?;
        let mut addr = self.server.clone();
        addr.set_port(port);

        match tokio::time::timeout(
            self.config.connect_timeout,
            self.conn_provider.new_data_connection(&addr, user_data),
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(_)) => Err(FtpTransferSetupError::DataTransferNotConnected),
            Err(_) => Err(FtpTransferSetupError::DataTransferConnectTimeout),
        }
    }

    async fn new_pasv_data_transfer<'a>(
        &'a mut self,
        user_data: &'a UD,
    ) -> Result<S, FtpTransferSetupError> {
        let sa = self.control.request_pasv_port().await?;
        let addr = UpstreamAddr::from_ip_and_port(sa.ip(), sa.port());

        match tokio::time::timeout(
            self.config.connect_timeout,
            self.conn_provider.new_data_connection(&addr, user_data),
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(_)) => Err(FtpTransferSetupError::DataTransferNotConnected),
            Err(_) => Err(FtpTransferSetupError::DataTransferConnectTimeout),
        }
    }

    async fn new_spsv_data_transfer<'a>(
        &'a mut self,
        user_data: &'a UD,
    ) -> Result<S, FtpTransferSetupError> {
        let identifier = self.control.request_spsv_identifier().await?;

        match tokio::time::timeout(
            self.config.connect_timeout,
            self.conn_provider
                .new_data_connection(&self.server, user_data),
        )
        .await
        {
            Ok(Ok(stream)) => {
                let mut control = FtpControlChannel::new(stream, self.config.control);
                match tokio::time::timeout(self.config.greeting_timeout, control.wait_greetings())
                    .await
                {
                    Ok(Ok(_)) => {
                        let stream = control.into_transfer_stream(&identifier).await?;
                        Ok(stream)
                    }
                    Ok(Err(e)) => Err(e.into()),
                    Err(_) => Err(FtpTransferSetupError::DataTransferConnectTimeout),
                }
            }
            Ok(Err(_)) => Err(FtpTransferSetupError::DataTransferNotConnected),
            Err(_) => Err(FtpTransferSetupError::DataTransferConnectTimeout),
        }
    }

    async fn new_data_transfer<'a>(
        &'a mut self,
        user_data: &'a UD,
    ) -> Result<S, FtpTransferSetupError> {
        if self.server_feature.support_epsv() || self.config.always_try_epsv {
            match self.new_epsv_data_transfer(user_data).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if e.skip_retry() {
                        return Err(e);
                    }
                }
            }
        }

        match self.new_pasv_data_transfer(user_data).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                if e.skip_retry() {
                    return Err(e);
                }
            }
        }

        if self.server_feature.support_spsv() {
            // NOTE there are possible implementations as mentioned in
            // https://datatracker.ietf.org/doc/html/draft-rosenau-ftp-single-port-05
            // we do not support those possible implementations
            // pure-ftpd has dropped it's other implementation in commit
            // https://github.com/jedisct1/pure-ftpd/commit/4828633d9cb42cd77d764e7d1cb3d0c04c5df001
            match self.new_spsv_data_transfer(user_data).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if e.skip_retry() {
                        return Err(e);
                    }
                }
            }
        }

        Err(FtpTransferSetupError::NeedActiveDataTransfer)
    }

    pub async fn abort_transfer(&mut self) -> Result<(), FtpCommandError> {
        self.control.abort_transfer().await
    }

    pub async fn list_directory_detailed_start<'a>(
        &'a mut self,
        path: &'a str,
        user_data: &'a UD,
    ) -> Result<S, FtpFileRetrieveStartError> {
        self.use_ascii_transfer().await?;

        if self.server_feature.support_pre_transfer() {
            match self.control.pre_list(path).await? {
                FtpFilePreTransferStatus::Proceed => {}
                FtpFilePreTransferStatus::Invalid => {
                    log_msg!("invalid pre transfer for list {}", path);
                }
            }
        }

        let data_stream = self.new_data_transfer(user_data).await?;

        self.control.start_list(path).await?;
        Ok(data_stream)
    }

    pub async fn list_directory_detailed_receive<'a, R>(
        &'a mut self,
        data_stream: S,
        receiver: &'a mut R,
    ) -> Result<(), FtpFileListError>
    where
        R: FtpLineDataReceiver,
    {
        tokio::pin! {
            let transfer_fut = FtpLineDataTransfer::new(data_stream, &self.config.transfer).read_to_end(receiver);
        }

        tokio::select! {
            biased;

            data = &mut transfer_fut => {
                tokio::time::timeout(self.config.transfer.end_wait_timeout, self.control.wait_list())
                    .await
                    .map_err(|_| FtpFileListError::TimeoutToWaitEndReply)??;
                if let Err(e) = data {
                    return Err(e.into());
                }
            }
            r = self.control.wait_list() => {
                if let Err(e) = r {
                    return Err(FtpFileListError::ServerReportedError(e));
                }
                tokio::time::timeout(self.config.transfer.end_wait_timeout, transfer_fut)
                    .await
                    .map_err(|_| FtpFileListError::TimeoutToWaitDataEof)??;
            }
            _ = tokio::time::sleep(self.config.transfer.list_all_timeout) => {
                return Err(FtpFileListError::TimeoutToWaitDataEof);
            }
        }

        Ok(())
    }

    async fn request_restart_transfer(&mut self, position: u64) -> Result<(), FtpCommandError> {
        if !self.server_feature.support_rest_stream() {
            return Err(FtpCommandError::CommandNotImplemented(FtpCommand::REST));
        }

        self.control.request_restart(position).await
    }

    pub async fn retrieve_file_start<'a>(
        &'a mut self,
        path: &'a str,
        offset: Option<u64>,
        user_data: &'a UD,
    ) -> Result<(S, Option<u64>), FtpFileRetrieveStartError> {
        self.use_binary_transfer().await?;

        let file_transfer_size = if self.server_feature.support_file_size() {
            self.control.request_size(path).await?
        } else {
            None
        };

        if self.server_feature.support_pre_transfer() {
            match self.control.pre_retrieve(path).await? {
                FtpFilePreTransferStatus::Proceed => {}
                FtpFilePreTransferStatus::Invalid => {
                    debug!("invalid pre transfer for retrieve {}", path);
                }
            }
        }

        let data_stream = self.new_data_transfer(user_data).await?;

        if let Some(offset) = offset {
            self.request_restart_transfer(offset).await?;
        }

        self.control.start_retrieve(path).await?;
        Ok((data_stream, file_transfer_size))
    }

    pub async fn store_file_start<'a>(
        &'a mut self,
        path: &'a str,
        user_data: &'a UD,
    ) -> Result<S, FtpFileStoreStartError> {
        self.use_binary_transfer().await?;

        if self.server_feature.support_pre_transfer() {
            match self.control.pre_store(path).await? {
                FtpFilePreTransferStatus::Proceed => {}
                FtpFilePreTransferStatus::Invalid => {
                    debug!("invalid pre transfer for store {}", path);
                }
            }
        }

        let data_stream = self.new_data_transfer(user_data).await?;

        self.control.start_store(path).await?;
        Ok(data_stream)
    }

    #[inline]
    pub fn transfer_end_wait_timeout(&self) -> Duration {
        self.config.transfer.end_wait_timeout
    }

    pub async fn wait_control_read_ready(&mut self) -> Result<(), FtpRawResponseError> {
        self.control.wait_read_ready().await
    }

    pub async fn wait_retrieve_end_reply(&mut self) -> Result<(), FtpFileRetrieveError> {
        match tokio::time::timeout(
            self.config.transfer.end_wait_timeout,
            self.control.wait_retrieve(),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(FtpFileRetrieveError::TimeoutToWaitEndReply),
        }
    }

    pub async fn wait_store_end_reply(&mut self) -> Result<(), FtpFileStoreError> {
        match tokio::time::timeout(
            self.config.transfer.end_wait_timeout,
            self.control.wait_store(),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(FtpFileStoreError::TimeoutToWaitEndReply),
        }
    }
}
