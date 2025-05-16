// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::utils::{derive_builtin_genesis_namespace, open_kanari_db};
use clap::Parser;
use kanari_anomalies::load_tx_anomalies;
use kanari_config::R_OPT_NET_HELP;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::{BuiltinChainID, KanariChainID};
use std::path::PathBuf;

/// Repair the database offline.
/// Help to reach consistency of the database.
#[derive(Debug, Parser)]
pub struct RepairCommand {
    #[clap(
        long,
        help = "perform a thorough and detailed check, which may take more time"
    )]
    pub thorough: bool,
    #[clap(
        long = "exec",
        help = "execute repair, otherwise only report issues. default is false"
    )]
    pub exec: bool,
    #[clap(
        long = "fast-fail",
        help = "fail fast on the first error, otherwise continue to check all issues"
    )]
    pub fast_fail: bool,
    #[clap(long = "sync-mode", help = "if true, no DA block will be generated")]
    pub sync_mode: bool,

    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: BuiltinChainID,
}

impl RepairCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let (_root, kanari_db, _start_time) = open_kanari_db(
            self.base_data_dir,
            Some(KanariChainID::Builtin(self.chain_id)),
        );

        let genesis_namespace = derive_builtin_genesis_namespace(self.chain_id)?;
        let tx_anomalies = load_tx_anomalies(genesis_namespace.clone())?;

        let (issues, fixed) = kanari_db.repair(
            self.thorough,
            self.exec,
            self.fast_fail,
            self.sync_mode,
            tx_anomalies,
        )?;

        println!("issues found: {}, fixed: {}", issues, fixed);

        Ok(())
    }
}
