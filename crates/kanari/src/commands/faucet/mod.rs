// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use async_trait::async_trait;
use clap::Parser;
use commands::server::ServerCommand;
use kanari_types::error::KanariResult;

pub mod commands;

/// Faucet commands
#[derive(Parser)]
pub struct Faucet {
    #[clap(subcommand)]
    cmd: FaucetCommand,
}

#[async_trait]
impl CommandAction<String> for Faucet {
    async fn execute(self) -> KanariResult<String> {
        match self.cmd {
            FaucetCommand::Server(server) => server.execute_serialized().await,
        }
    }
}

#[derive(clap::Subcommand)]
#[clap(name = "faucet")]
pub enum FaucetCommand {
    Server(ServerCommand),
}
