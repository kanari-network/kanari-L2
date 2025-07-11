// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_rpc_api::jsonrpc_types::ObjectStateView;
use kanari_types::{address::ParsedAddress, error::KanariResult};
use move_command_line_common::types::ParsedStructType;
use moveos_types::access_path::AccessPath;

#[derive(Debug, Parser)]

/// Get account resource by tag
pub struct ResourceCommand {
    /// Account address where the resource stored.
    #[clap(long, value_parser=ParsedAddress::parse)]
    pub address: ParsedAddress,

    /// Struct name as `<ADDRESS>::<MODULE_ID>::<STRUCT_NAME><TypeParam1?, TypeParam2?>`
    /// Example: `0x123::counter::Counter`, `0x123::counter::Box<0x123::counter::Counter>`
    #[clap(long = "resource", value_parser=ParsedStructType::parse)]
    pub resource: ParsedStructType,

    #[clap(flatten)]
    pub(crate) context_options: WalletContextOptions,

    /// Render and return display fields.
    #[clap(long)]
    pub show_display: bool,
}

#[async_trait]
impl CommandAction<Option<ObjectStateView>> for ResourceCommand {
    async fn execute(self) -> KanariResult<Option<ObjectStateView>> {
        let context = self.context_options.build()?;
        let mapping = context.address_mapping();
        let address = self.address.into_account_address(&mapping)?;
        let resource = self.resource.into_struct_tag(&mapping)?;
        let client = context.get_client().await?;

        let resp = if self.show_display {
            client
                .kanari
                .get_decoded_states_with_display(AccessPath::resource(address, resource))
                .await?
                .pop()
                .flatten()
        } else {
            client
                .kanari
                .get_decoded_states(AccessPath::resource(address, resource), None)
                .await?
                .pop()
                .flatten()
        };
        Ok(resp)
    }
}
