// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use crate::commands::upgrade::commands::upgrade_gas_config::UpgradeGasConfigCommand;

use async_trait::async_trait;
use clap::{Parser, Subcommand};
use kanari_types::error::KanariResult;

pub mod commands;

/// Tool for interacting with system upgrade
#[derive(Parser)]
pub struct Upgrade {
    #[clap(subcommand)]
    cmd: UpgradeCommand,
}

#[async_trait]
impl CommandAction<String> for Upgrade {
    async fn execute(self) -> KanariResult<String> {
        match self.cmd {
            UpgradeCommand::UpgradeGasConfig(cmd) => cmd.execute_serialized().await,
        }
    }
}

#[derive(Subcommand)]
pub enum UpgradeCommand {
    UpgradeGasConfig(UpgradeGasConfigCommand),
}
