// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::CommandAction;
use crate::commands::move_cli::serialized_success;
use async_trait::async_trait;
use clap::Parser;
use kanari_types::error::KanariResult;
use move_cli::{Move, base::disassemble::Disassemble};
use serde_json::Value;

/// Disassemble the Move bytecode pointed to
#[derive(Parser)]
#[clap(name = "disassemble")]
pub struct DisassembleCommand {
    #[clap(flatten)]
    pub disassemble: Disassemble,

    #[clap(flatten)]
    move_args: Move,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<Value>> for DisassembleCommand {
    async fn execute(self) -> KanariResult<Option<Value>> {
        let path = self.move_args.package_path;
        let config = self.move_args.build_config;
        self.disassemble.execute(path, config)?;

        serialized_success(self.json)
    }
}
