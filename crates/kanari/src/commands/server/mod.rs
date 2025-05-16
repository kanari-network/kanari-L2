// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use async_trait::async_trait;
use clap::Parser;
use commands::start::StartCommand;
use kanari_types::error::KanariResult;

use self::commands::clean::CleanCommand;

pub mod commands;

/// Start Kanari network
#[derive(Parser)]
pub struct Server {
    #[clap(subcommand)]
    cmd: ServerCommand,
}

#[async_trait]
impl CommandAction<String> for Server {
    async fn execute(self) -> KanariResult<String> {
        match self.cmd {
            ServerCommand::Start(start) => start.execute_serialized().await,
            ServerCommand::Clean(clean) => clean.execute().map(|_| "".to_owned()),
        }
    }
}

#[derive(clap::Subcommand)]
#[clap(name = "server")]
pub enum ServerCommand {
    Start(StartCommand),
    Clean(CleanCommand),
}
