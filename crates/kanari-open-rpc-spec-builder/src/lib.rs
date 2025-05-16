// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::clap_derive::ValueEnum;
use clap::Parser;
use kanari_open_rpc::Project;
use kanari_rpc_api::api::btc_api::BtcAPIOpenRpc;
use kanari_rpc_api::api::kanari_api::KanariAPIOpenRpc;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

mod examples;

pub fn kanari_rpc_doc(version: &str) -> Project {
    Project::new(
        version,
        "Kanari JSON-RPC",
        "Kanari JSON-RPC API for interaction with kanari server. ",
        "Kanari Network",
        "https://kanari.site",
        "opensource@kanari.site",
        "Apache-2.0",
        "https://raw.githubusercontent.com/kanari-network/kanari/main/LICENSE",
    )
}

#[derive(Debug, Parser, Clone, Copy, ValueEnum)]
enum Action {
    Print,
    Test,
    Record,
}
// TODO: This currently always use workspace version, which is not ideal.
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn build_kanari_rpc_spec() -> Project {
    let mut open_rpc = kanari_rpc_doc(VERSION);
    open_rpc.add_module(KanariAPIOpenRpc::module_doc());
    //FIXME if add the EthAPIOpenRpc, the pnpm sdk gen raies error
    open_rpc.add_module(BtcAPIOpenRpc::module_doc());
    //open_rpc.add_examples(RpcExampleProvider::new().examples());
    open_rpc
}

pub fn build_and_save_kanari_rpc_spec() -> Result<()> {
    let open_rpc = build_kanari_rpc_spec();
    let content = serde_json::to_string_pretty(&open_rpc)?;
    let mut f = File::create(spec_file()).unwrap();
    writeln!(f, "{content}")?;
    Ok(())
}

pub fn spec_file() -> PathBuf {
    path_in_crate("../kanari-open-rpc-spec/schemas/openrpc.json")
}

fn path_in_crate<S>(relative: S) -> PathBuf
where
    S: AsRef<Path>,
{
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(relative);
    path
}
