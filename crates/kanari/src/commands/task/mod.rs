// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use async_trait::async_trait;
use commands::schedule::ScheduleCommand;
use kanari_types::error::KanariResult;

pub mod commands;

/// Tool for run some task
#[derive(clap::Parser)]
pub struct Task {
    #[clap(subcommand)]
    cmd: TaskCommand,
}

#[async_trait]
impl CommandAction<String> for Task {
    async fn execute(self) -> KanariResult<String> {
        match self.cmd {
            TaskCommand::Schedule(schedule) => schedule.execute_serialized().await,
        }
    }
}

#[derive(Debug, clap::Subcommand)]
#[clap(name = "task")]
pub enum TaskCommand {
    Schedule(ScheduleCommand),
}
