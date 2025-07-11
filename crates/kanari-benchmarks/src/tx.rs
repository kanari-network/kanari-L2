// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::config::TxType;
use crate::tx::TxType::{Empty, Transfer, TransferLargeObject};
use anyhow::Result;
use bitcoin::consensus::deserialize;
use bitcoin::hashes::Hash;
use bitcoin::hex::FromHex;
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc_json::bitcoin;
use bitcoincore_rpc_json::bitcoin::Block;
use kanari_sequencer::actor::sequencer::SequencerActor;
use kanari_store::KanariStore;
use kanari_test_transaction_builder::TestTransactionBuilder;
use kanari_types::crypto::KanariKeyPair;
use kanari_types::multichain_id::KanariMultiChainID;
use kanari_types::service_status::ServiceStatus;
use kanari_types::transaction::L1BlockWithBody;
use kanari_types::transaction::kanari::KanariTransaction;
use prometheus::Registry;
use std::fs;
use std::path::Path;
use tracing::info;

pub const EXAMPLE_SIMPLE_BLOG_PACKAGE_NAME: &str = "simple_blog";
pub const EXAMPLE_SIMPLE_BLOG_NAMED_ADDRESS: &str = "simple_blog";

pub fn gen_sequencer(
    keypair: KanariKeyPair,
    kanari_store: KanariStore,
    registry: &Registry,
) -> Result<SequencerActor> {
    SequencerActor::new(
        keypair,
        kanari_store.clone(),
        ServiceStatus::Active,
        registry,
        None,
    )
}

pub fn create_publish_transaction(
    test_transaction_builder: &TestTransactionBuilder,
) -> Result<KanariTransaction> {
    let publish_action = test_transaction_builder.new_publish_examples(
        EXAMPLE_SIMPLE_BLOG_PACKAGE_NAME,
        Some(EXAMPLE_SIMPLE_BLOG_NAMED_ADDRESS.to_string()),
    )?;
    test_transaction_builder.build_and_sign(publish_action)
}

pub fn create_l2_tx(
    test_transaction_builder: &mut TestTransactionBuilder,
    seq_num: u64,
    tx_type: TxType,
) -> Result<KanariTransaction> {
    test_transaction_builder.update_sequence_number(seq_num);

    let action = match tx_type {
        Empty => test_transaction_builder.call_empty_create(),
        Transfer => test_transaction_builder.call_transfer_create(),
        TransferLargeObject => test_transaction_builder.call_transfer_large_object_create(),
        _ => panic!("Unsupported tx type"),
    };

    test_transaction_builder.build_and_sign(action)
}

pub fn find_block_height(dir: &Path) -> Result<Vec<u64>> {
    let mut block_heights = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "hex" {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let height: u64 = file_stem
                .parse()
                .expect("Failed to parse block height from filename");
            block_heights.push(height);
        }
    }

    block_heights.sort();
    Ok(block_heights)
}

pub fn create_btc_blk_tx(height: u64, block_file: &Path) -> Result<L1BlockWithBody> {
    let block_hex_str = fs::read_to_string(block_file)?;
    let block_hex = Vec::<u8>::from_hex(&block_hex_str)?;
    let origin_block: Block = deserialize(&block_hex)?;
    let block = origin_block.clone();
    let block_hash = block.header.block_hash();
    let move_block = kanari_types::bitcoin::types::Block::from(block.clone());
    Ok(L1BlockWithBody {
        block: kanari_types::transaction::L1Block {
            chain_id: KanariMultiChainID::Bitcoin.multichain_id(),
            block_height: height,
            block_hash: block_hash.to_byte_array().to_vec(),
        },
        block_body: move_block.encode(),
    })
}

// Download btc block data via bitcoin client
pub fn prepare_btc_block(
    btc_block_dir: &Path,
    btc_rpc_url: String,
    btc_rpc_username: String,
    btc_rpc_password: String,
    btc_block_start_height: u64,
    btc_block_count: u64,
) {
    if !btc_block_dir.exists() {
        fs::create_dir_all(btc_block_dir).unwrap();
    }

    let client = bitcoincore_rpc::Client::new(
        btc_rpc_url.as_str(),
        bitcoincore_rpc::Auth::UserPass(btc_rpc_username, btc_rpc_password),
    )
    .unwrap();

    for i in 0..btc_block_count {
        let height = btc_block_start_height + i;
        let filename = format!("{}.hex", height);
        let file_path = btc_block_dir.join(filename);

        if file_path.exists() {
            continue;
        }

        let block_hash = client.get_block_hash(height).unwrap();
        let block_hex = client.get_block_hex(&block_hash).unwrap();
        info!("Downloaded block {} to {}", height, file_path.display());
        fs::write(file_path, block_hex).unwrap();
    }
}
