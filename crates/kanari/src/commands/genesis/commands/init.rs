// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use kanari_config::{KanariOpt, R_OPT_NET_HELP};
use kanari_db::KanariDB;
use kanari_genesis::KanariGenesisV2;
use kanari_types::{
    error::{KanariError, KanariResult},
    kanari_network::KanariChainID,
};
use metrics::RegistryService;
use std::path::PathBuf;

/// Init genesis statedb
#[derive(Debug, Parser)]
pub struct InitCommand {
    #[clap(long = "data-dir", short = 'd')]
    /// Path to data dir, this dir is base dir, the final data_dir is base_dir/chain_network_name
    pub base_data_dir: Option<PathBuf>,

    /// If local chainid, start the service with a temporary data store.
    /// All data will be deleted when the service is stopped.
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,

    #[clap(long)]
    /// The genesis config file path for custom chain network.
    /// If the file path equals to builtin chain network name(local/dev/test/main), will use builtin genesis config.
    pub genesis_config: Option<String>,
}

impl InitCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let opt =
            KanariOpt::new_with_default(self.base_data_dir, self.chain_id, self.genesis_config)?;
        let store_config = opt.store_config();
        let registry_service = RegistryService::default();
        let kanari_db = KanariDB::init(store_config, &registry_service.default_registry())?;
        let network = opt.network();
        let _genesis = KanariGenesisV2::load_or_init(network, &kanari_db)?;
        let root = kanari_db
            .latest_root()?
            .ok_or_else(|| KanariError::from(anyhow::anyhow!("Load latest root failed")))?;
        println!(
            "Genesis statedb initialized at {:?} successfully, state_root: {:?}",
            opt.base().data_dir(),
            root.state_root()
        );
        Ok(())
    }
}
