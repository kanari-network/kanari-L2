// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use itertools::Itertools;
use kanari_rpc_api::jsonrpc_types::{FieldKeyView, ObjectStateView, StateOptions};
use kanari_types::{error::KanariResult, function_arg::ParsedObjectID};
use moveos_types::state::FieldKey;

/// Get field states for a special dynamic field with an Object ID and Field Keys.
#[derive(Debug, Parser)]
pub struct GetFieldStatesCommand {
    #[clap(short = 'i', long, required = true)]
    object_id: ParsedObjectID,

    #[clap(short = 'k', long, required = true)]
    field_keys: Vec<FieldKey>,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[async_trait]
impl CommandAction<Vec<Option<ObjectStateView>>> for GetFieldStatesCommand {
    async fn execute(self) -> KanariResult<Vec<Option<ObjectStateView>>> {
        let context = self.context_options.build()?;
        let address_mapping = context.address_mapping();
        let client = context.get_client().await?;

        let object_id = self.object_id.into_object_id(&address_mapping)?;
        let options = Some(StateOptions::new().decode(true));
        let field_keys_view = self
            .field_keys
            .iter()
            .map(|k| <FieldKey as Into<FieldKeyView>>::into(*k))
            .collect_vec();

        let resp = client
            .kanari
            .get_field_states(object_id.into(), field_keys_view, options)
            .await?;

        Ok(resp)
    }
}
