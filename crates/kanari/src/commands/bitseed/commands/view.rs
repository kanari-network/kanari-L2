// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use clap::Parser;
use kanari_types::{
    bitcoin::ord::InscriptionID,
    error::{KanariError, KanariResult},
};

use crate::{
    cli_types::{CommandAction, WalletContextOptions},
    commands::bitseed::{
        operation::{AsSFT, Operation},
        sft::SFT,
    },
};

#[derive(Debug, Parser)]
pub struct ViewCommand {
    #[arg(long, help = "The SFT inscription ID to view.")]
    sft_inscription_id: InscriptionID,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[async_trait]
impl CommandAction<SFT> for ViewCommand {
    async fn execute(self) -> KanariResult<SFT> {
        let context = self.context_options.build()?;
        let client = context.get_client().await?;
        let ins_obj_id = self.sft_inscription_id.object_id();

        let ins_obj = client
            .kanari
            .get_inscription_object(ins_obj_id.clone())
            .await?
            .ok_or_else(|| {
                KanariError::CommandArgumentError(format!(
                    "Inscription object {} not found",
                    ins_obj_id
                ))
            })?;

        let operation = Operation::from_inscription(ins_obj.value.into())?;
        let sft = match operation {
            Operation::Mint(mint_record) => mint_record.as_sft(),
            Operation::Split(split_record) => split_record.as_sft(),
            Operation::Merge(merge_record) => merge_record.as_sft(),
            _ => {
                return Err(KanariError::CommandArgumentError(format!(
                    "Inscription {} is not a valid SFT record",
                    self.sft_inscription_id
                )))
            }
        };

        Ok(sft)
    }
}
