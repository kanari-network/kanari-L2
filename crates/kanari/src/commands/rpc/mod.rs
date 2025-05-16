// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use async_trait::async_trait;
use clap::Parser;
use commands::request::RequestCommand;
use kanari_types::error::KanariResult;

pub mod commands;

#[derive(Parser)]
pub struct Rpc {
    #[clap(subcommand)]
    cmd: RpcCommand,
}

#[async_trait]
impl CommandAction<String> for Rpc {
    async fn execute(self) -> KanariResult<String> {
        match self.cmd {
            RpcCommand::Request(request) => request.execute_serialized().await,
        }
    }
}

#[derive(clap::Subcommand)]
#[clap(name = "server")]
pub enum RpcCommand {
    Request(RequestCommand),
}
