/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::path::PathBuf;

use anyhow::anyhow;
use capnp_rpc::{RpcSystem, rpc_twoparty_capnp, twoparty};
use clap::{Arg, ArgMatches, Command, ValueHint, value_parser};
use clap_complete::Shell;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

#[cfg(unix)]
const DEFAULT_TMP_CONTROL_DIR: &str = "/tmp/g3";

const GLOBAL_ARG_COMPLETION: &str = "completion";
const GLOBAL_ARG_CONTROL_DIR: &str = "control-dir";
const GLOBAL_ARG_GROUP: &str = "daemon-group";
const GLOBAL_ARG_PID: &str = "pid";

pub trait DaemonCtlArgsExt {
    fn append_daemon_ctl_args(self) -> Self;
}

#[derive(Debug, Default)]
pub struct DaemonCtlArgs {
    shell_completion: Option<Shell>,
    #[cfg(unix)]
    control_dir: Option<PathBuf>,
    daemon_group: String,
    pid: usize,
}

impl DaemonCtlArgs {
    pub fn parse_clap(args: &ArgMatches) -> Self {
        let mut config = DaemonCtlArgs::default();

        if let Some(shell) = args.get_one::<Shell>(GLOBAL_ARG_COMPLETION) {
            config.shell_completion = Some(*shell);
            return config;
        }

        #[cfg(unix)]
        if let Some(dir) = args.get_one::<PathBuf>(GLOBAL_ARG_CONTROL_DIR) {
            config.control_dir = Some(dir.clone());
        }

        if let Some(group) = args.get_one::<String>(GLOBAL_ARG_GROUP) {
            config.daemon_group.clone_from(group);
        }

        if let Some(pid) = args.get_one::<usize>(GLOBAL_ARG_PID) {
            config.pid = *pid;
        }

        config
    }

    pub fn generate_shell_completion<F>(&mut self, build_cmd: F) -> bool
    where
        F: Fn() -> Command,
    {
        let Some(shell) = self.shell_completion.take() else {
            return false;
        };
        let mut cmd = build_cmd();
        let bin_name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
        true
    }

    pub async fn connect_rpc<T>(
        &self,
        daemon_name: &str,
    ) -> anyhow::Result<(RpcSystem<rpc_twoparty_capnp::Side>, T)>
    where
        T: capnp::capability::FromClientHook,
    {
        let stream = self.connect_to_daemon(daemon_name).await?;

        let (reader, writer) = tokio::io::split(stream);
        let reader = tokio_util::compat::TokioAsyncReadCompatExt::compat(reader);
        let writer = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(writer);
        let rpc_network = Box::new(twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        ));
        let mut rpc_system = RpcSystem::new(rpc_network, None);
        let client: T = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
        Ok((rpc_system, client))
    }

    #[cfg(unix)]
    async fn connect_to_daemon(
        &self,
        daemon_name: &str,
    ) -> anyhow::Result<impl AsyncRead + AsyncWrite + use<>> {
        let control_dir = self.control_dir.clone().unwrap_or_else(|| {
            let mut sys_ctl_dir = PathBuf::from("/run");
            sys_ctl_dir.push(daemon_name);

            if sys_ctl_dir.is_dir() {
                sys_ctl_dir
            } else {
                PathBuf::from(DEFAULT_TMP_CONTROL_DIR)
            }
        });
        let socket_path = if self.pid != 0 {
            control_dir.join(format!("{}_{}.sock", self.daemon_group, self.pid))
        } else {
            control_dir.join(format!("{}.sock", self.daemon_group))
        };

        let mut stream = tokio::net::UnixStream::connect(&socket_path)
            .await
            .map_err(|e| {
                anyhow!(
                    "failed to connect to control socket {}: {e:?}",
                    socket_path.display()
                )
            })?;
        self.enter_rpc_mode(&mut stream).await?;
        Ok(stream)
    }

    #[cfg(windows)]
    async fn connect_to_daemon(
        &self,
        daemon_name: &str,
    ) -> anyhow::Result<impl AsyncRead + AsyncWrite + use<>> {
        let pipe_name = if self.pid != 0 {
            format!(r"\\.\pipe\{daemon_name}@{}:{}", self.daemon_group, self.pid)
        } else {
            format!(r"\\.\pipe\{daemon_name}@{}", self.daemon_group)
        };

        let mut stream = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(&pipe_name)
            .map_err(|e| anyhow!("failed to open connection to pipe {}: {e:?}", pipe_name))?;
        self.enter_rpc_mode(&mut stream).await?;
        Ok(stream)
    }

    async fn enter_rpc_mode<S>(&self, stream: &mut S) -> anyhow::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        stream
            .write_all(b"capnp\n")
            .await
            .map_err(|e| anyhow!("enter capnp mode failed: {e:?}"))?;
        stream
            .flush()
            .await
            .map_err(|e| anyhow!("enter capnp mod failed: {e:?}"))?;
        Ok(())
    }
}

impl DaemonCtlArgsExt for Command {
    fn append_daemon_ctl_args(self) -> Self {
        self.arg(
            Arg::new(GLOBAL_ARG_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_CONTROL_DIR)
                .help("Directory that contains the control socket")
                .value_name("CONTROL DIR")
                .value_hint(ValueHint::DirPath)
                .value_parser(value_parser!(PathBuf))
                .short('C')
                .long("control-dir"),
        )
        .arg(
            Arg::new(GLOBAL_ARG_GROUP)
                .required_unless_present_any([GLOBAL_ARG_PID, GLOBAL_ARG_COMPLETION])
                .num_args(1)
                .value_name("GROUP NAME")
                .help("Daemon group name")
                .short('G')
                .long("daemon-group"),
        )
        .arg(
            Arg::new(GLOBAL_ARG_PID)
                .help("Daemon pid")
                .required_unless_present_any([GLOBAL_ARG_GROUP, GLOBAL_ARG_COMPLETION])
                .num_args(1)
                .value_name("PID")
                .value_parser(value_parser!(usize))
                .short('p')
                .long("daemon-pid"),
        )
    }
}
