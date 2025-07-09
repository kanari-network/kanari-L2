// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use crate::commands::move_cli::print_serialized_success;
use async_trait::async_trait;
use clap::Parser;
use kanari_config::KANARI_CLIENT_CONFIG;
use kanari_framework::{KANARI_FRAMEWORK_ADDRESS, KANARI_FRAMEWORK_ADDRESS_NAME};
use kanari_types::error::{KanariError, KanariResult};
use move_cli::{Move, base::new};
use move_core_types::account_address::AccountAddress;
use moveos_types::addresses::{
    MOVE_STD_ADDRESS, MOVE_STD_ADDRESS_NAME, MOVEOS_STD_ADDRESS, MOVEOS_STD_ADDRESS_NAME,
};
use serde_json::Value;

const MOVE_STDLIB_PKG_NAME: &str = "MoveStdlib";
const MOVE_STDLIB_PKG_PATH: &str = "{ git = \"https://github.com/kanari-network/kanari-L2.git\", subdir = \"frameworks/move-stdlib\", rev = \"kanari-network\" }";

const MOVEOS_STDLIB_PKG_NAME: &str = "MoveosStdlib";
const MOVEOS_STDLIB_PKG_PATH: &str = "{ git = \"https://github.com/kanari-network/kanari-L2.git\", subdir = \"frameworks/moveos-stdlib\", rev = \"kanari-network\" }";

const KANARI_FRAMEWORK_PKG_NAME: &str = "KanariFramework";
const KANARI_FRAMEWORK_PKG_PATH: &str = "{ git = \"https://github.com/kanari-network/kanari-L2.git\", subdir = \"frameworks/kanari-framework\", rev = \"kanari-network\" }";

#[derive(Parser)]
pub struct NewCommand {
    /// Existing account address from Kanari
    #[clap(long = "address", short = 'a')]
    account_address: Option<AccountAddress>,

    ///The name of the package to be created.
    name: String,

    #[clap(flatten)]
    wallet_context_options: WalletContextOptions,

    #[clap(flatten)]
    move_args: Move,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

impl NewCommand {
    async fn get_active_account_address_from_config(&self) -> Result<String, KanariError> {
        // build wallet context options
        let context = self.wallet_context_options.build()?;
        // get active account address value
        match context.client_config.active_address {
            Some(address) => Ok(AccountAddress::from(address).to_hex_literal()),
            None => Err(KanariError::ConfigLoadError(
                KANARI_CLIENT_CONFIG.to_string(),
                format!(
                    "No active address found in {}. Check if {} is complete",
                    KANARI_CLIENT_CONFIG, KANARI_CLIENT_CONFIG,
                ),
            )),
        }
    }
}

#[async_trait]
impl CommandAction<Option<Value>> for NewCommand {
    async fn execute(self) -> KanariResult<Option<Value>> {
        let path = self.move_args.package_path.clone();

        let name = &self.name.to_lowercase();
        let address = if let Some(account_address) = &self.account_address {
            // Existing account address is available
            account_address.to_hex_literal()
        } else {
            // Existing account address is not available, use the active address from config file generated from the command `kanari init`
            match self.get_active_account_address_from_config().await {
                Ok(active_account_address) => active_account_address,
                Err(err) => return Err(err),
            }
        };

        let new_cli = new::New {
            name: name.to_string(),
        };
        new_cli.execute(
            path,
            "0.0.1",
            [
                (MOVE_STDLIB_PKG_NAME, MOVE_STDLIB_PKG_PATH),
                (MOVEOS_STDLIB_PKG_NAME, MOVEOS_STDLIB_PKG_PATH),
                (KANARI_FRAMEWORK_PKG_NAME, KANARI_FRAMEWORK_PKG_PATH),
            ],
            [
                (name, &address),
                (
                    &MOVE_STD_ADDRESS_NAME.to_string(),
                    &MOVE_STD_ADDRESS.to_hex_literal(),
                ),
                (
                    &MOVEOS_STD_ADDRESS_NAME.to_string(),
                    &MOVEOS_STD_ADDRESS.to_hex_literal(),
                ),
                (
                    &KANARI_FRAMEWORK_ADDRESS_NAME.to_string(),
                    &KANARI_FRAMEWORK_ADDRESS.to_hex_literal(),
                ),
            ],
            "",
        )?;

        print_serialized_success(self.json)
    }
}
