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

use std::net::SocketAddr;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use tokio::io::{AsyncRead, AsyncWrite, BufStream};

use g3_io_ext::LimitedBufReadExt;
use g3_types::auth::{Password, Username};

use crate::error::{
    FtpAuthStatus, FtpCommandError, FtpFilePreTransferStatus, FtpFileRetrieveStartError,
    FtpFileStatError, FtpFileStoreStartError, FtpRawResponseError, FtpTransferServerError,
};
use crate::facts::time_val;
use crate::feature::FtpServerFeature;
use crate::transfer::FtpTransferType;
use crate::{FtpControlConfig, FtpFileFacts};

mod response;

mod command;
pub(crate) use command::FtpCommand;

pub(crate) struct FtpControlChannel<T>
where
    T: AsyncRead + AsyncWrite,
{
    config: FtpControlConfig,
    stream: BufStream<T>,
}

impl<T> FtpControlChannel<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(stream: T, config: FtpControlConfig) -> Self {
        FtpControlChannel {
            config,
            stream: BufStream::new(stream),
        }
    }

    pub(crate) async fn wait_read_ready(&mut self) -> Result<(), FtpRawResponseError> {
        match self.stream.fill_wait_data().await {
            Ok(true) => Ok(()),
            Ok(false) => Err(FtpRawResponseError::ConnectionClosed),
            Err(e) => Err(FtpRawResponseError::ReadFailed(e)),
        }
    }

    pub(crate) async fn into_transfer_stream(
        mut self,
        identifier: &str,
    ) -> Result<T, FtpCommandError> {
        let cmd = FtpCommand::SPDT;
        self.send_cmd1(cmd, identifier)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("into transfer stream").await?;
        match reply.code() {
            200 => Ok(self.stream.into_inner()),
            504 => Err(FtpCommandError::ParameterNotImplemented(cmd)),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn wait_greetings(&mut self) -> Result<(), FtpCommandError> {
        loop {
            let reply = self.read_raw_response().await?;
            return match reply.code() {
                120 => continue,
                220 => Ok(()),
                421 => Err(FtpCommandError::ServiceNotAvailable),
                n => Err(FtpCommandError::UnexpectedReplyCode(
                    FtpCommand::GREETING,
                    n,
                )),
            };
        }
    }

    pub(crate) async fn check_server_feature(
        &mut self,
    ) -> Result<FtpServerFeature, FtpCommandError> {
        let mut feature = FtpServerFeature::default();

        let cmd = FtpCommand::FEAT;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("check server feature").await?;
        match reply.code() {
            500 | 501 | 502 => {}
            211 => {
                if let Some(lines) = reply.lines() {
                    for line in &lines[1..] {
                        if line.as_bytes()[0] != b' ' {
                            break;
                        }
                        feature.parse_and_set(line.trim());
                    }
                }
            }
            421 => return Err(FtpCommandError::ServiceNotAvailable),
            n => return Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }

        Ok(feature)
    }

    pub(crate) async fn set_use_utf8(&mut self) -> Result<bool, FtpCommandError> {
        let cmd = FtpCommand::OPTS_UTF8_ON;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("set use utf8").await?;
        match reply.code() {
            500 | 501 | 502 => Ok(false),
            200 => Ok(true),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn send_username(
        &mut self,
        name: Option<&Username>,
    ) -> Result<FtpAuthStatus, FtpCommandError> {
        let cmd = FtpCommand::USER;
        let username = name.map(|u| u.as_original()).unwrap_or("anonymous");
        self.send_cmd1(cmd, username)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("send username").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            530 => Ok(FtpAuthStatus::NotLoggedIn),
            230 => Ok(FtpAuthStatus::LoggedIn),
            331 => Ok(FtpAuthStatus::NeedPassword),
            332 => Ok(FtpAuthStatus::NeedAccount),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn send_password(
        &mut self,
        pass: Option<&Password>,
    ) -> Result<FtpAuthStatus, FtpCommandError> {
        let cmd = FtpCommand::PASS;
        let password = pass.map(|p| p.as_original()).unwrap_or("xxx");
        self.send_cmd1(FtpCommand::PASS, password)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("send password").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            503 => Err(FtpCommandError::BadCommandSequence(cmd)),
            530 => Ok(FtpAuthStatus::NotLoggedIn),
            202 => Err(FtpCommandError::CommandNotImplemented(cmd)), // not fatal but unexpected
            230 => Ok(FtpAuthStatus::LoggedIn),
            332 => Ok(FtpAuthStatus::NeedAccount),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn send_quit(&mut self) -> Result<(), FtpCommandError> {
        let cmd = FtpCommand::QUIT;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("send quit").await?;
        match reply.code() {
            500 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            221 => Ok(()),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn delete_file(&mut self, path: &str) -> Result<(), FtpFileStatError> {
        let cmd = FtpCommand::DELE;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("delete file")
            .await
            .map_err(FtpCommandError::RecvFailed)?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd).into()),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd).into()),
            530 => Err(FtpCommandError::NotLoggedIn.into()),
            550 => Err(FtpFileStatError::FileUnavailable),
            250 => Ok(()),
            421 => Err(FtpFileStatError::ServiceNotAvailable),
            450 => Err(FtpFileStatError::FileUnavailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n).into()),
        }
    }

    pub(crate) async fn remove_dir(&mut self, path: &str) -> Result<(), FtpFileStatError> {
        let cmd = FtpCommand::RMD;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("remove dir")
            .await
            .map_err(FtpCommandError::RecvFailed)?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd).into()),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd).into()),
            530 => Err(FtpCommandError::NotLoggedIn.into()),
            550 => Err(FtpFileStatError::FileUnavailable),
            250 => Ok(()),
            421 => Err(FtpFileStatError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n).into()),
        }
    }

    pub(crate) async fn request_mlst(
        &mut self,
        path: &str,
    ) -> Result<Option<FtpFileFacts>, FtpCommandError> {
        let cmd = FtpCommand::MLST;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request mlst").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            504 => Err(FtpCommandError::ParameterNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            550 => Ok(None),
            250 => {
                if let Some(lines) = reply.lines() {
                    if lines.len() == 3 {
                        let line = &lines[1];
                        if let Ok(ff) = FtpFileFacts::parse_line(line.as_str()) {
                            return Ok(Some(ff));
                        }
                    }
                }

                Err(FtpCommandError::InvalidReplySyntax(cmd, 250))
            }
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_size(
        &mut self,
        path: &str,
    ) -> Result<Option<u64>, FtpCommandError> {
        if path.is_empty() {
            return Ok(None);
        }

        let cmd = FtpCommand::SIZE;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request size").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            550 => Ok(None),
            213 => {
                if let Some(s) = reply.line_trimmed() {
                    let size = u64::from_str(s)
                        .map_err(|_| FtpCommandError::InvalidReplySyntax(cmd, 213))?;
                    Ok(Some(size))
                } else {
                    Err(FtpCommandError::InvalidReplySyntax(cmd, 213))
                }
            }
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_mtime(
        &mut self,
        path: &str,
    ) -> Result<Option<DateTime<Utc>>, FtpCommandError> {
        let cmd = FtpCommand::MDTM;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request mtime").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            550 => Ok(None),
            213 => {
                if let Some(s) = reply.line_trimmed() {
                    let mtime = time_val::parse_from_str(s)
                        .map_err(|_| FtpCommandError::InvalidReplySyntax(cmd, 213))?;
                    Ok(Some(mtime))
                } else {
                    Err(FtpCommandError::InvalidReplySyntax(cmd, 213))
                }
            }
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_pasv_port(&mut self) -> Result<SocketAddr, FtpCommandError> {
        let cmd = FtpCommand::PASV;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request pasv port").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            227 => match reply.parse_pasv_227_reply() {
                Some(addr) => Ok(addr),
                None => Err(FtpCommandError::InvalidReplySyntax(cmd, 227)),
            },
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_epsv_port(&mut self) -> Result<u16, FtpCommandError> {
        let cmd = FtpCommand::EPSV;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request epsv port").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            522 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            229 => match reply.parse_epsv_229_reply() {
                Some(port) => Ok(port),
                None => Err(FtpCommandError::InvalidReplySyntax(cmd, 229)),
            },
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_spsv_identifier(&mut self) -> Result<String, FtpCommandError> {
        let cmd = FtpCommand::SPSV;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("request spsv identifier")
            .await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            522 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            227 => match reply.parse_spsv_227_reply() {
                Some(identifier) => Ok(identifier),
                None => Err(FtpCommandError::InvalidReplySyntax(cmd, 227)),
            },
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn abort_transfer(&mut self) -> Result<(), FtpCommandError> {
        let cmd = FtpCommand::ABOR;
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("abort transfer").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            226 => Ok(()),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            426 => {
                let reply = self.timed_read_raw_response("wait abort transfer").await?;
                match reply.code() {
                    226 => Ok(()),
                    n => {
                        // use 1xxx to represent the second one of reply code
                        Err(FtpCommandError::UnexpectedReplyCode(cmd, 1000 + n))
                    }
                }
            }
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn request_transfer_type(
        &mut self,
        t: FtpTransferType,
    ) -> Result<(), FtpCommandError> {
        let cmd = match t {
            FtpTransferType::Ascii => FtpCommand::TYPE_A,
            FtpTransferType::Image => FtpCommand::TYPE_I,
        };
        self.send_cmd(cmd)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("request transfer type")
            .await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            504 => Err(FtpCommandError::ParameterNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            200 => Ok(()),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    async fn wait_pre_transfer_reply(
        &mut self,
        cmd: FtpCommand,
    ) -> Result<FtpFilePreTransferStatus, FtpCommandError> {
        let reply = self
            .timed_read_raw_response("wait pre transfer reply")
            .await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(FtpCommand::PRET)),
            502 => Err(FtpCommandError::CommandNotImplemented(FtpCommand::PRET)),
            530 => Err(FtpCommandError::NotLoggedIn),
            550 => Ok(FtpFilePreTransferStatus::Invalid),
            200 => Ok(FtpFilePreTransferStatus::Proceed),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::PreTransferFailed(cmd, n)), // including 550
        }
    }

    pub(crate) async fn pre_list(
        &mut self,
        path: &str,
    ) -> Result<FtpFilePreTransferStatus, FtpCommandError> {
        let cmd = FtpCommand::LIST;
        self.send_pre_transfer_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;
        self.wait_pre_transfer_reply(cmd).await
    }

    pub(crate) async fn start_list(&mut self, path: &str) -> Result<(), FtpFileRetrieveStartError> {
        let cmd = FtpCommand::LIST;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("start list")
            .await
            .map_err(FtpCommandError::RecvFailed)?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd).into()),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd).into()),
            530 => Err(FtpCommandError::NotLoggedIn.into()),
            125 | 150 => Ok(()),
            421 => Err(FtpFileRetrieveStartError::ServiceNotAvailable),
            450 => Err(FtpFileRetrieveStartError::FileUnavailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n).into()),
        }
    }

    pub(crate) async fn wait_list(&mut self) -> Result<(), FtpTransferServerError> {
        let reply = self.read_raw_response().await?;
        match reply.code() {
            226 | 250 => Ok(()),
            425 => Err(FtpTransferServerError::DataTransferNotEstablished),
            426 => Err(FtpTransferServerError::DataTransferLost),
            451 => Err(FtpTransferServerError::ServerFailed),
            n => Err(FtpTransferServerError::UnexpectedEndReplyCode(
                FtpCommand::LIST,
                n,
            )),
        }
    }

    pub(crate) async fn request_restart(&mut self, position: u64) -> Result<(), FtpCommandError> {
        let cmd = FtpCommand::REST;
        self.send_cmd1(cmd, &position.to_string())
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self.timed_read_raw_response("request restart").await?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd)),
            502 => Err(FtpCommandError::CommandNotImplemented(cmd)),
            530 => Err(FtpCommandError::NotLoggedIn),
            350 => Ok(()),
            421 => Err(FtpCommandError::ServiceNotAvailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n)),
        }
    }

    pub(crate) async fn pre_retrieve(
        &mut self,
        path: &str,
    ) -> Result<FtpFilePreTransferStatus, FtpCommandError> {
        let cmd = FtpCommand::RETR;
        self.send_pre_transfer_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;
        self.wait_pre_transfer_reply(cmd).await
    }

    pub(crate) async fn start_retrieve(
        &mut self,
        path: &str,
    ) -> Result<(), FtpFileRetrieveStartError> {
        let cmd = FtpCommand::RETR;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("start retrieve")
            .await
            .map_err(FtpCommandError::RecvFailed)?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd).into()),
            530 => Err(FtpCommandError::NotLoggedIn.into()),
            550 => Err(FtpFileRetrieveStartError::FileUnavailable),
            125 | 150 => Ok(()),
            421 => Err(FtpFileRetrieveStartError::ServiceNotAvailable),
            450 => Err(FtpFileRetrieveStartError::FileUnavailable),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n).into()),
        }
    }

    pub(crate) async fn wait_retrieve(&mut self) -> Result<(), FtpTransferServerError> {
        let reply = self.read_raw_response().await?;
        match reply.code() {
            110 => Err(FtpTransferServerError::RestartNeeded),
            226 | 250 => Ok(()),
            425 => Err(FtpTransferServerError::DataTransferNotEstablished),
            426 => Err(FtpTransferServerError::DataTransferLost),
            451 => Err(FtpTransferServerError::ServerFailed),
            n => Err(FtpTransferServerError::UnexpectedEndReplyCode(
                FtpCommand::RETR,
                n,
            )),
        }
    }

    pub(crate) async fn pre_store(
        &mut self,
        path: &str,
    ) -> Result<FtpFilePreTransferStatus, FtpCommandError> {
        let cmd = FtpCommand::STOR;
        self.send_pre_transfer_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;
        self.wait_pre_transfer_reply(cmd).await
    }

    pub(crate) async fn start_store(&mut self, path: &str) -> Result<(), FtpFileStoreStartError> {
        let cmd = FtpCommand::STOR;
        self.send_cmd1(cmd, path)
            .await
            .map_err(FtpCommandError::SendFailed)?;

        let reply = self
            .timed_read_raw_response("start store")
            .await
            .map_err(FtpCommandError::RecvFailed)?;
        match reply.code() {
            500 | 501 => Err(FtpCommandError::RejectedCommandSyntax(cmd).into()),
            530 => Err(FtpCommandError::NotLoggedIn.into()),
            532 => Err(FtpFileStoreStartError::NeedAccountForStoring),
            553 => Err(FtpFileStoreStartError::FileNameNotAllowed),
            125 | 150 => Ok(()),
            421 => Err(FtpFileStoreStartError::ServiceNotAvailable),
            450 => Err(FtpFileStoreStartError::FileUnavailable),
            452 => Err(FtpFileStoreStartError::InsufficientStorageSpace),
            n => Err(FtpCommandError::UnexpectedReplyCode(cmd, n).into()),
        }
    }

    pub(crate) async fn wait_store(&mut self) -> Result<(), FtpTransferServerError> {
        let reply = self.read_raw_response().await?;
        match reply.code() {
            110 => Err(FtpTransferServerError::RestartNeeded),
            226 | 250 => Ok(()),
            425 => Err(FtpTransferServerError::DataTransferNotEstablished),
            426 => Err(FtpTransferServerError::DataTransferLost),
            451 => Err(FtpTransferServerError::ServerFailed),
            551 => Err(FtpTransferServerError::PageTypeUnknown),
            552 => Err(FtpTransferServerError::ExceededStorageAllocation),
            n => Err(FtpTransferServerError::UnexpectedEndReplyCode(
                FtpCommand::STOR,
                n,
            )),
        }
    }
}
