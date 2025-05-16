// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use clap::Parser;
use kanari_genesis::{genesis_file, KanariGenesis, KanariGenesisV2};
use kanari_types::kanari_network::{BuiltinChainID, KanariNetwork};
use tracing::info;

#[derive(Parser)]
#[clap(name = "genesis-release", author = "The Kanari Core Contributors")]
struct GenesisOpts {
    /// The builtin chain id for the genesis
    #[clap(long, short = 'n', default_value = "test")]
    chain_id: BuiltinChainID,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    let opts: GenesisOpts = GenesisOpts::parse();
    match &opts.chain_id {
        BuiltinChainID::Test | BuiltinChainID::Main => {}
        _ => {
            bail!(
                "chain_id {:?} is not supported, only support release test and main",
                opts.chain_id
            );
        }
    }
    info!("start to build genesis for chain: {:?}", opts.chain_id);
    let network: KanariNetwork = KanariNetwork::builtin(opts.chain_id);
    let genesis = KanariGenesisV2::build(network)?;
    // Ensure testnet and mainnet genesis file use old format
    let genesis_v1 = KanariGenesis::from(genesis);
    let genesis_file = genesis_file(opts.chain_id);
    genesis_v1.save_to(genesis_file)?;
    Ok(())
}
