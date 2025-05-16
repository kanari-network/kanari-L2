// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use kanari_config::da_config::derive_namespace_from_genesis;
use kanari_genesis::{KanariGenesis, KanariGenesisV2};
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::{BuiltinChainID, KanariNetwork};
use std::path::PathBuf;

/// Derive DA namespace from genesis file.
#[derive(Debug, Parser)]
pub struct NamespaceCommand {
    #[clap(long)]
    genesis_file: Option<PathBuf>,
    #[clap(long, short = 'n', default_value = "test")]
    chain_id: Option<BuiltinChainID>,
}

impl NamespaceCommand {
    pub fn execute(self) -> KanariResult<()> {
        let genesis = if let Some(genesis_file) = self.genesis_file {
            load_genesis_from_file(genesis_file)?
        } else {
            KanariGenesisV2::load_or_build(KanariNetwork::builtin(self.chain_id.unwrap()))?
        };

        let genesis_v1 = KanariGenesis::from(genesis);
        let genesis_hash = genesis_v1.genesis_hash();
        let namespace = derive_namespace_from_genesis(genesis_hash);
        println!("namespace: {}", namespace);
        let encoded_hash = hex::encode(genesis_hash.0);
        println!("genesis hash: {}", encoded_hash);
        Ok(())
    }
}

fn load_genesis_from_file(path: PathBuf) -> anyhow::Result<KanariGenesisV2> {
    let contents = std::fs::read(path)?;
    KanariGenesisV2::decode(&contents)
}
