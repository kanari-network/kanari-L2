// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use accumulator::accumulator_info::AccumulatorInfo;
use accumulator::{Accumulator, MerkleAccumulator};
use anyhow::anyhow;
use heed::byteorder::BigEndian;
use heed::types::{SerdeBincode, U64};
use heed::{Database, Env, EnvOpenOptions};
use kanari_anomalies::TxAnomalies;
use kanari_config::KanariOpt;
use kanari_db::KanariDB;
use kanari_rpc_client::Client;
use kanari_store::KanariStore;
use kanari_types::crypto::KanariKeyPair;
use kanari_types::da::batch::DABatch;
use kanari_types::da::chunk::{Chunk, ChunkV0, chunk_from_segments};
use kanari_types::da::segment::{SegmentID, segment_from_bytes};
use kanari_types::kanari_network::KanariChainID;
use kanari_types::sequencer::SequencerInfo;
use kanari_types::transaction::{LedgerTransaction, TransactionSequenceInfo};
use metrics::RegistryService;
use moveos_store::transaction_store::{TransactionDBStore, TransactionStore};
use moveos_types::h256::H256;
use moveos_types::moveos_std::object::ObjectMeta;
use moveos_types::transaction::TransactionExecutionInfo;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, watch};
use tokio::time;
use tracing::{error, info, warn};

pub mod accumulator_anomaly;
pub mod exec;
pub mod find_first;
pub mod index;
pub mod namespace;
pub mod pack;
pub mod repair;
pub mod unpack;
pub mod verify;

const DEFAULT_MAX_SEGMENT_SIZE: usize = 4 * 1024 * 1024;

pub(crate) struct SequencedTxStore {
    last_sequenced_tx_order_in_last_job: u64,
    tx_accumulator: MerkleAccumulator,
    kanari_store: KanariStore,
    tx_anomalies: Option<TxAnomalies>,
}

impl SequencedTxStore {
    pub(crate) fn new(
        kanari_store: KanariStore,
        tx_anomalies: Option<TxAnomalies>,
    ) -> anyhow::Result<Self> {
        // The sequencer info would be initialized when genesis, so the sequencer info should not be None
        let last_sequencer_info = kanari_store
            .get_meta_store()
            .get_sequencer_info()?
            .ok_or_else(|| anyhow::anyhow!("Load sequencer info failed"))?;
        let (last_sequenced_tx_order_in_last_job, last_accumulator_info) = (
            last_sequencer_info.last_order,
            last_sequencer_info.last_accumulator_info.clone(),
        );
        info!(
            "Load latest sequencer order {:?}",
            last_sequenced_tx_order_in_last_job
        );
        info!(
            "Load latest sequencer accumulator info {:?}",
            last_accumulator_info
        );
        let tx_accumulator = MerkleAccumulator::new_with_info(
            last_accumulator_info,
            kanari_store.get_transaction_accumulator_store(),
        );

        Ok(SequencedTxStore {
            tx_accumulator,
            kanari_store,
            last_sequenced_tx_order_in_last_job,
            tx_anomalies,
        })
    }

    pub(crate) fn get_last_sequenced_tx_order_in_last_job(&self) -> u64 {
        self.last_sequenced_tx_order_in_last_job
    }

    pub(crate) fn save_tx(&self, mut tx: LedgerTransaction) -> anyhow::Result<()> {
        let tx_order = tx.sequence_info.tx_order;
        if let Some(tx_anomalies) = &self.tx_anomalies {
            if let Some(tx_hash_should_revert) =
                tx_anomalies.get_accumulator_should_revert(tx_order)
            {
                self.tx_accumulator
                    .append(vec![tx_hash_should_revert].as_slice())?;
                info!(
                    "append tx_hash_should_revert: {:?}, tx_order: {}",
                    tx_hash_should_revert, tx_order
                );
            }
        }

        let tx_hash = tx.tx_hash();

        let _tx_accumulator_root = self.tx_accumulator.append(vec![tx_hash].as_slice())?;
        let tx_accumulator_unsaved_nodes = self.tx_accumulator.pop_unsaved_nodes();
        let tx_accumulator_info = self.tx_accumulator.get_info();

        let exp_accumulator_root = tx.sequence_info.tx_accumulator_root;
        let exp_accumulator_info = AccumulatorInfo {
            accumulator_root: exp_accumulator_root,
            frozen_subtree_roots: tx.sequence_info.tx_accumulator_frozen_subtree_roots.clone(),
            num_leaves: tx.sequence_info.tx_accumulator_num_leaves,
            num_nodes: tx.sequence_info.tx_accumulator_num_nodes,
        };

        if tx_accumulator_info != exp_accumulator_info {
            return Err(anyhow::anyhow!(
                "Tx accumulator mismatch for tx_order: {}, tx_hash: {:?}, expect: {:?}, actual: {:?}",
                tx_order,
                tx_hash,
                exp_accumulator_info,
                tx_accumulator_info
            ));
        }

        let sequencer_info = SequencerInfo::new(tx_order, tx_accumulator_info);
        self.kanari_store.save_sequenced_tx(
            tx_hash,
            tx,
            sequencer_info,
            tx_accumulator_unsaved_nodes,
            true,
        )?;
        self.tx_accumulator.clear_after_save();
        Ok(())
    }
}

pub(crate) fn collect_chunk(segment_dir: PathBuf, chunk_id: u128) -> anyhow::Result<Vec<u64>> {
    let mut segments = Vec::new();
    for segment_number in 0.. {
        let segment_id = SegmentID {
            chunk_id,
            segment_number,
        };
        let segment_path = segment_dir.join(segment_id.to_string());
        if !segment_path.exists() {
            if segment_number == 0 {
                return Err(anyhow::anyhow!("No segment found in chunk: {}", chunk_id));
            } else {
                break;
            }
        }

        segments.push(segment_number);
    }
    Ok(segments)
}

// collect all the chunks from segment_dir.
// each segment is stored in a file named by the segment_id.
// each chunk may contain multiple segments.
// we collect all the chunks and their segment numbers to unpack them later.
pub(crate) fn collect_chunks(
    segment_dir: PathBuf,
    allow_empty: bool,
) -> anyhow::Result<(HashMap<u128, Vec<u64>>, u128, u128)> {
    let mut chunks = HashMap::new();
    let mut max_chunk_id = 0;
    let mut min_chunk_id = u128::MAX;
    for entry in fs::read_dir(segment_dir.clone())?.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(segment_id) = path
                .file_name()
                .and_then(|s| s.to_str()?.parse::<SegmentID>().ok())
            {
                let chunk_id = segment_id.chunk_id;
                let segment_number = segment_id.segment_number;
                let segments: &mut Vec<u64> = chunks.entry(chunk_id).or_default();
                segments.push(segment_number);
                if chunk_id > max_chunk_id {
                    max_chunk_id = chunk_id;
                }
                if chunk_id < min_chunk_id {
                    min_chunk_id = chunk_id;
                }
            }
        }
    }

    let origin_chunk_count = chunks.len();
    // remove chunks that don't have segment_number 0
    // because we need to start from segment_number 0 to unpack the chunk.
    // in the download process, we download segments to tmp dir first,
    // then move them to segment dir,
    // a segment with segment_number 0 is the last segment to move,
    // so if it exists, the chunk is complete.
    let chunks: HashMap<u128, Vec<u64>> =
        chunks.into_iter().filter(|(_, v)| v.contains(&0)).collect();
    let chunk_count = chunks.len();
    if chunk_count < origin_chunk_count {
        error!(
            "Removed {} incomplete chunks, {} chunks left. Please check the segment dir: {:?} and download the missing segments.",
            origin_chunk_count - chunk_count,
            chunk_count,
            segment_dir
        );
        return Err(anyhow::anyhow!("Incomplete chunks found"));
    }

    if chunks.is_empty() && !allow_empty {
        return Err(anyhow::anyhow!(
            "No segment found in {:?}. allow empty: {}",
            segment_dir,
            allow_empty
        ));
    }
    Ok((chunks, min_chunk_id, max_chunk_id))
}

pub(crate) fn get_tx_list_from_chunk(
    segment_dir: PathBuf,
    chunk_id: u128,
    segment_numbers: Vec<u64>,
    verify_order: bool,
) -> anyhow::Result<Vec<LedgerTransaction>> {
    let mut segments = Vec::new();
    for segment_number in segment_numbers {
        let segment_id = SegmentID {
            chunk_id,
            segment_number,
        };
        let segment_path = segment_dir.join(segment_id.to_string());
        let segment_bytes = fs::read(segment_path)?;
        let segment = segment_from_bytes(&segment_bytes)?;
        segments.push(segment);
    }
    let chunk = chunk_from_segments(segments)?;
    let batch = chunk.get_batches().into_iter().next().unwrap();
    batch.verify(verify_order)?;
    batch.get_tx_list()
}

pub(crate) fn build_kanari_db(
    base_data_dir: Option<PathBuf>,
    chain_id: Option<KanariChainID>,
    enable_rocks_stats: bool,
    row_cache_size: Option<u64>,
    block_cache_size: Option<u64>,
) -> (ObjectMeta, KanariDB) {
    let mut opt = KanariOpt::new_with_default(base_data_dir, chain_id, None).unwrap();
    opt.store.enable_statistics = enable_rocks_stats;
    opt.store.row_cache_size = row_cache_size;
    opt.store.block_cache_size = block_cache_size;
    let registry_service = RegistryService::default();
    let kanari_db =
        KanariDB::init(opt.store_config(), &registry_service.default_registry()).unwrap();
    let root = kanari_db.latest_root().unwrap().unwrap();
    (root, kanari_db)
}

pub(crate) struct SegmentDownloader {
    open_da_path: String,
    segment_dir: PathBuf,
    next_chunk_id: u128,
    chunks: Arc<RwLock<HashMap<u128, Vec<u64>>>>,
}

impl SegmentDownloader {
    pub(crate) fn new(
        open_da_path: String,
        segment_dir: PathBuf,
        next_chunk_id: u128,
        chunks: Arc<RwLock<HashMap<u128, Vec<u64>>>>,
    ) -> anyhow::Result<Self> {
        Ok(SegmentDownloader {
            open_da_path,
            segment_dir,
            next_chunk_id,
            chunks,
        })
    }

    async fn download_chunk(
        open_da_path: String,
        segment_dir: PathBuf,
        segment_tmp_dir: PathBuf,
        chunk_id: u128,
    ) -> anyhow::Result<Option<Vec<u64>>> {
        let tmp_dir = segment_tmp_dir;
        let mut done_segments = Vec::new();
        for segment_number in 0.. {
            let segment_url = format!("{}/{}_{}", open_da_path, chunk_id, segment_number);
            let res = reqwest::get(segment_url).await?;
            if res.status().is_success() {
                let segment_bytes = res.bytes().await?;
                let segment_path = tmp_dir.join(format!("{}_{}", chunk_id, segment_number));
                let mut file = File::create(&segment_path)?;
                file.write_all(&segment_bytes)?;
                done_segments.push(segment_number);
            } else {
                if res.status() == StatusCode::NOT_FOUND {
                    if segment_number == 0 {
                        return Ok(None);
                    } else {
                        break; // no more segments for this chunk
                    }
                }
                return Err(anyhow!(
                    "Failed to download segment: {}_{}: {} ",
                    chunk_id,
                    segment_number,
                    res.status(),
                ));
            }
        }

        for segment_number in done_segments.clone().into_iter().rev() {
            let tmp_path = tmp_dir.join(format!("{}_{}", chunk_id, segment_number));
            let dst_path = segment_dir.join(format!("{}_{}", chunk_id, segment_number));
            fs::rename(tmp_path, dst_path)?;
        }

        Ok(Some(done_segments))
    }

    pub(crate) fn run_in_background(
        self,
        shutdown_signal: watch::Receiver<()>,
    ) -> anyhow::Result<()> {
        let base_url = self.open_da_path;
        let segment_dir = self.segment_dir;
        let tmp_dir = segment_dir.join("tmp");
        fs::create_dir_all(&tmp_dir)?;
        let next_chunk_id = self.next_chunk_id;

        tokio::spawn(async move {
            let mut shutdown_signal = shutdown_signal;

            let mut interval = time::interval(Duration::from_secs(60 * 5));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            let mut chunk_id = next_chunk_id;
            let base_url = base_url.clone();

            loop {
                tokio::select! {
                    _ = shutdown_signal.changed() => {
                        info!("Shutting down segments download task.");
                        break;
                    }
                    _ = interval.tick() => {
                        loop {
                            let res = Self::download_chunk(base_url.clone(), segment_dir.clone(), tmp_dir.clone(), chunk_id).await;
                            match res {
                                Ok(Some(segments)) => {
                                    let mut chunks = self.chunks.write().await;
                                    chunks.insert(chunk_id, segments);
                                    chunk_id += 1;
                                }
                                Err(e) => {
                                    warn!("Failed to download chunk: {}, error: {}", chunk_id, e);
                                    break;
                                }
                            _ => {
                                break;
                                }}
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

pub(crate) struct ExpRoots {
    inner: Arc<RwLock<ExpRootsInner>>,
}

impl Clone for ExpRoots {
    fn clone(&self) -> Self {
        ExpRoots {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct ExpRootsInner {
    exp_roots: HashMap<u64, H256>,
    exp_roots_file: File,
    max_tx_order: u64,
}

impl ExpRoots {
    pub(crate) async fn get_max_tx_order(&self) -> u64 {
        self.inner.read().await.max_tx_order
    }

    pub(crate) async fn new(exp_roots_path: PathBuf) -> anyhow::Result<Self> {
        let mut exp_roots = HashMap::new();
        let file: File;
        let mut max_tx_order = 0;

        if exp_roots_path.exists() {
            let mut reader = BufReader::new(File::open(&exp_roots_path)?);
            for line in reader.by_ref().lines() {
                let line = line?;
                let parts: Vec<&str> = line.split(':').collect();
                let tx_order = parts[0].parse::<u64>()?;
                let state_root_raw = parts[1];
                if state_root_raw == "null" {
                    continue;
                }
                let state_root = H256::from_str(state_root_raw)?;
                exp_roots.insert(tx_order, state_root);

                if tx_order > max_tx_order {
                    max_tx_order = tx_order;
                }
            }
            file = File::options()
                .append(true)
                .create(true)
                .open(&exp_roots_path)?;
        } else {
            file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&exp_roots_path)?;
        }

        Ok(ExpRoots {
            inner: Arc::new(RwLock::new(ExpRootsInner {
                exp_roots,
                exp_roots_file: file,
                max_tx_order,
            })),
        })
    }

    /// Insert a new exp root for the given tx_order
    /// Updates both the in-memory map and the file
    pub(crate) async fn insert(&self, tx_order: u64, state_root: H256) -> anyhow::Result<()> {
        let mut inner = self.inner.write().await;

        let mut update_file = true;
        // Update in-memory map
        let old_state = inner.exp_roots.insert(tx_order, state_root);
        if let Some(old) = old_state {
            if old != state_root {
                warn!(
                    "Overwriting existing exp root for tx_order {}: {:?} -> {:?}",
                    tx_order, old, state_root
                );
            } else {
                update_file = false; // No need to update file if the state root is the same
            }
        }

        // Update max_tx_order if necessary
        if tx_order > inner.max_tx_order {
            inner.max_tx_order = tx_order;
        }

        // Update file
        if !update_file {
            return Ok(());
        }
        let entry = format!("{}:{:?}\n", tx_order, state_root);
        inner.exp_roots_file.write_all(entry.as_bytes())?;
        inner.exp_roots_file.flush()?;

        Ok(())
    }

    /// Get the exp root for the given tx_order
    /// Returns None if no exp root exists for the tx_order
    pub(crate) async fn get(&self, tx_order: u64) -> Option<H256> {
        self.inner.read().await.exp_roots.get(&tx_order).copied()
    }
}

pub(crate) struct LedgerTxGetter {
    segment_dir: PathBuf,
    chunks: Arc<RwLock<HashMap<u128, Vec<u64>>>>,
    max_chunk_id: u128,
}

impl LedgerTxGetter {
    pub(crate) fn new(segment_dir: PathBuf, allow_empty: bool) -> anyhow::Result<Self> {
        let (chunks, _min_chunk_id, max_chunk_id) =
            collect_chunks(segment_dir.clone(), allow_empty)?;

        Ok(LedgerTxGetter {
            segment_dir,
            chunks: Arc::new(RwLock::new(chunks)),
            max_chunk_id,
        })
    }

    pub(crate) fn new_with_auto_sync(
        open_da_path: String,
        segment_dir: PathBuf,
        shutdown_signal: watch::Receiver<()>,
    ) -> anyhow::Result<Self> {
        let (chunks, _min_chunk_id, max_chunk_id) = collect_chunks(segment_dir.clone(), true)?;

        let chunks_to_sync = Arc::new(RwLock::new(chunks.clone()));

        let downloader = SegmentDownloader::new(
            open_da_path,
            segment_dir.clone(),
            max_chunk_id + 1,
            chunks_to_sync.clone(),
        )?;
        downloader.run_in_background(shutdown_signal)?;
        Ok(LedgerTxGetter {
            segment_dir,
            chunks: chunks_to_sync,
            max_chunk_id,
        })
    }

    pub(crate) async fn load_ledger_tx_list(
        &self,
        chunk_id: u128,
        must_has: bool,
        verify_order: bool,
    ) -> anyhow::Result<Option<Vec<LedgerTransaction>>> {
        let tx_list_opt = self
            .chunks
            .read()
            .await
            .get(&chunk_id)
            .cloned()
            .map_or_else(
                || {
                    if must_has {
                        Err(anyhow::anyhow!("No segment found in chunk {}", chunk_id))
                    } else {
                        Ok(None)
                    }
                },
                |segment_numbers| {
                    let tx_list = get_tx_list_from_chunk(
                        self.segment_dir.clone(),
                        chunk_id,
                        segment_numbers.clone(),
                        verify_order,
                    )?;
                    Ok(Some(tx_list))
                },
            )?;

        Ok(tx_list_opt)
    }

    // only valid for no segments sync
    pub(crate) fn get_max_chunk_id(&self) -> u128 {
        self.max_chunk_id
    }
}

pub(crate) struct StateRootFetcher {
    client: Client,
    exp_roots: ExpRoots,
    tx_anomalies: Option<TxAnomalies>,
}

impl StateRootFetcher {
    pub(crate) fn new(
        client: Client,
        exp_roots: ExpRoots,
        tx_anomalies: Option<TxAnomalies>,
    ) -> Self {
        StateRootFetcher {
            client,
            exp_roots,
            tx_anomalies,
        }
    }

    pub(crate) async fn fetch_and_add(&self, tx_order: u64) -> anyhow::Result<()> {
        let state_root = self.fetch(tx_order).await?;
        if let Some(state_root) = state_root {
            self.exp_roots.insert(tx_order, state_root).await?;
        }
        Ok(())
    }

    pub(crate) async fn fetch(&self, tx_order: u64) -> anyhow::Result<Option<H256>> {
        let resp = self
            .client
            .kanari
            .get_transactions_by_order(Some(tx_order - 1), Some(1), Some(false))
            .await?;
        let resp_date = resp.data;
        if resp_date.is_empty() {
            return Err(anyhow!(
                "No transaction found by RPC. tx_order: {}",
                tx_order,
            ));
        }
        let tx_info = resp_date[0].clone();
        let tx_order_in_resp = tx_info.transaction.sequence_info.tx_order.0;
        if tx_order_in_resp != tx_order {
            Err(anyhow!(
                "failed to request tx by RPC: Tx order mismatch, expect: {}, actual: {}",
                tx_order,
                tx_order_in_resp
            ))
        } else {
            let execution_info_opt = tx_info.execution_info;
            if let Some(execution_info) = execution_info_opt {
                let tx_state_root = execution_info.state_root.0;
                return Ok(Some(tx_state_root));
            } else if let Some(tx_anomalies) = self.tx_anomalies.as_ref() {
                if tx_anomalies.has_no_execution_info_for_order(tx_order) {
                    return Ok(None);
                };
            }
            Err(anyhow!(
                "No state_root found by PRC. tx_order: {}",
                tx_order,
            ))
        }
    }
}

pub(crate) struct TxMetaStore {
    tx_position_indexer: TxPositionIndexer,
    exp_roots: ExpRoots, // tx_order -> (state_root, accumulator_root)
    max_exp_tx_order_from_file: u64,
    transaction_store: TransactionDBStore,
    kanari_store: KanariStore,
}

impl TxMetaStore {
    pub(crate) async fn new(
        tx_position_indexer_path: PathBuf,
        exp_roots_path: PathBuf,
        segment_dir: PathBuf,
        transaction_store: TransactionDBStore,
        kanari_store: KanariStore,
        max_block_number: Option<u128>,
    ) -> anyhow::Result<Self> {
        let tx_position_indexer = TxPositionIndexer::new_with_updates(
            tx_position_indexer_path,
            None,
            Some(segment_dir),
            max_block_number,
        )
        .await?;
        let exp_roots = ExpRoots::new(exp_roots_path).await?;
        let max_exp_tx_order_from_file = exp_roots.get_max_tx_order().await;

        Ok(TxMetaStore {
            tx_position_indexer,
            exp_roots,
            max_exp_tx_order_from_file,
            transaction_store,
            kanari_store,
        })
    }

    pub(crate) fn get_exp_roots(&self) -> ExpRoots {
        self.exp_roots.clone()
    }

    pub(crate) async fn get_exp_state_root(&self, tx_order: u64) -> Option<H256> {
        self.exp_roots.get(tx_order).await
    }

    pub(crate) fn get_max_verified_tx_order(&self) -> u64 {
        self.max_exp_tx_order_from_file
    }

    pub(crate) fn get_tx_hash(&self, tx_order: u64) -> Option<H256> {
        let r = self
            .tx_position_indexer
            .get_tx_position(tx_order)
            .ok()
            .flatten();
        r.map(|tx_position| tx_position.tx_hash)
    }

    pub(crate) fn get_tx_positions_in_range(
        &self,
        start_tx_order: u64,
        end_tx_order: u64,
    ) -> anyhow::Result<Vec<TxPosition>> {
        self.tx_position_indexer
            .get_tx_positions_in_range(start_tx_order, end_tx_order)
    }

    pub(crate) fn find_last_executed(&self) -> anyhow::Result<Option<TxPosition>> {
        let predicate = |tx_order: &u64| self.has_executed_by_tx_order(*tx_order);
        let last_tx_order = self.tx_position_indexer.last_tx_order;
        if last_tx_order == 0 {
            // no tx indexed through DA segments
            return Ok(None);
        }
        if !predicate(&1) {
            return Ok(None); // first tx in DA segments is not executed
        }
        if predicate(&last_tx_order) {
            return self.tx_position_indexer.get_tx_position(last_tx_order); // last tx is executed
        }

        // binary search [1, self.tx_position_indexer.last_tx_order]
        let mut left = 1; // first tx is executed, has checked
        let mut right = last_tx_order;

        while left + 1 < right {
            let mid = left + (right - left) / 2;
            if predicate(&mid) {
                left = mid; // mid is true, the final answer is mid or on the right
            } else {
                right = mid; // mid is false, the final answer is on the left
            }
        }

        // left is the last true position
        self.tx_position_indexer.get_tx_position(left)
    }

    pub(crate) fn find_tx_block(&self, tx_order: u64) -> Option<u128> {
        let r = self
            .tx_position_indexer
            .get_tx_position(tx_order)
            .ok()
            .flatten();
        r.map(|tx_position| tx_position.block_number)
    }

    fn has_executed_by_tx_order(&self, tx_order: u64) -> bool {
        self.get_tx_hash(tx_order)
            .map_or(false, |tx_hash| self.has_executed(tx_hash))
    }

    fn has_executed(&self, tx_hash: H256) -> bool {
        self.get_execution_info(tx_hash)
            .map_or(false, |info| info.is_some())
    }

    pub(crate) fn get_execution_info(
        &self,
        tx_hash: H256,
    ) -> anyhow::Result<Option<TransactionExecutionInfo>> {
        self.transaction_store.get_tx_execution_info(tx_hash)
    }

    pub(crate) fn get_sequencer_info(
        &self,
        tx_hash: H256,
    ) -> anyhow::Result<Option<TransactionSequenceInfo>> {
        Ok(self
            .kanari_store
            .transaction_store
            .get_transaction_by_hash(tx_hash)?
            .map(|transaction| transaction.sequence_info))
    }
}

const MAP_SIZE: usize = 1 << 34; // 16G
const MAX_DBS: u32 = 1;
const ORDER_DATABASE_NAME: &str = "order_db";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct TxPosition {
    pub(crate) tx_order: u64,
    pub(crate) tx_hash: H256,
    pub(crate) block_number: u128,
}

pub(crate) struct TxPositionIndexer {
    db_env: Env,
    db: Database<U64<BigEndian>, SerdeBincode<TxPosition>>,
    last_tx_order: u64,
    last_block_number: u128,
}

#[derive(Debug, Serialize)]
pub(crate) struct TxPositionIndexerStats {
    pub(crate) total_tx_count: u64,
    pub(crate) last_tx_order: u64,
    pub(crate) last_block_number: u128,
}

impl TxPositionIndexer {
    pub(crate) fn load_or_dump(
        db_path: PathBuf,
        file_path: PathBuf,
        dump: bool,
    ) -> anyhow::Result<()> {
        if dump {
            let indexer = TxPositionIndexer::new(db_path, None)?;
            indexer.dump_to_file(file_path)
        } else {
            TxPositionIndexer::load_from_file(db_path, file_path)
        }
    }

    pub(crate) fn dump_to_file(&self, file_path: PathBuf) -> anyhow::Result<()> {
        let db = self.db;
        let file = File::create(file_path)?;
        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file.try_clone()?);
        let rtxn = self.db_env.read_txn()?;
        let mut iter = db.iter(&rtxn)?;
        while let Some((k, v)) = iter.next().transpose()? {
            writeln!(writer, "{}:{:?}:{}", k, v.tx_hash, v.block_number)?;
        }
        drop(iter);
        rtxn.commit()?;
        writer.flush().expect("Unable to flush writer");
        file.sync_data().expect("Unable to sync file");
        Ok(())
    }

    pub(crate) fn load_from_file(db_path: PathBuf, file_path: PathBuf) -> anyhow::Result<()> {
        let mut last_tx_order = 0;
        let mut last_tx_hash = H256::zero();
        let mut last_block_number = 0;

        let db_env = Self::create_env(db_path.clone())?;
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut wtxn = db_env.write_txn()?; // Begin write_transaction early for create/put

        let mut is_verify = false;
        let db: Database<U64<BigEndian>, SerdeBincode<TxPosition>> =
            match db_env.open_database(&wtxn, Some(ORDER_DATABASE_NAME)) {
                Ok(Some(db)) => {
                    info!("Database already exists, verify mode");
                    is_verify = true;
                    db
                }
                Ok(None) => db_env.create_database(&mut wtxn, Some(ORDER_DATABASE_NAME))?,
                Err(e) => return Err(e.into()), // Proper error propagation
            };
        wtxn.commit()?;

        let mut wtxn = db_env.write_txn()?;

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() != 3 {
                return Err(anyhow!("invalid line: {}", line));
            }
            let tx_order = parts[0].parse::<u64>()?;
            let tx_hash = H256::from_str(parts[1])?;
            let block_number = parts[2].parse::<u128>()?;
            let tx_position = TxPosition {
                tx_order,
                tx_hash,
                block_number,
            };

            if is_verify {
                let rtxn = db_env.read_txn()?;
                let ret = db.get(&rtxn, &tx_order)?;
                let ret = ret.ok_or(anyhow!("tx_order not found: {}", tx_order))?;
                rtxn.commit()?;
                assert_eq!(ret, tx_position);
            } else {
                db.put(&mut wtxn, &tx_order, &tx_position)?;
            }

            last_tx_order = tx_order;
            last_tx_hash = tx_hash;
            last_block_number = block_number;
        }

        wtxn.commit()?;

        if last_tx_order != 0 {
            let rtxn = db_env.read_txn()?;
            let ret = db.last(&rtxn)?;
            assert_eq!(
                ret,
                Some((
                    last_tx_order,
                    TxPosition {
                        tx_order: last_tx_order,
                        tx_hash: last_tx_hash,
                        block_number: last_block_number,
                    }
                ))
            );
        }

        {
            let rtxn = db_env.read_txn()?;
            let final_count = db.iter(&rtxn)?.count();
            info!("Final record count: {}", final_count);
            rtxn.commit()?;
        }

        db_env.force_sync()?;

        Ok(())
    }

    pub(crate) fn new(db_path: PathBuf, reset_from: Option<u64>) -> anyhow::Result<Self> {
        let db_env = Self::create_env(db_path)?;
        let mut txn = db_env.write_txn()?;
        let db: Database<U64<BigEndian>, SerdeBincode<TxPosition>> =
            db_env.create_database(&mut txn, Some(ORDER_DATABASE_NAME))?;
        txn.commit()?;

        let mut indexer = TxPositionIndexer {
            db_env,
            db,
            last_tx_order: 0,
            last_block_number: 0,
        };
        if let Some(from) = reset_from {
            indexer.reset_from(from)?;
        }

        indexer.init_cursor()?;
        Ok(indexer)
    }

    pub(crate) async fn new_with_updates(
        db_path: PathBuf,
        reset_from: Option<u64>,
        segment_dir: Option<PathBuf>,
        max_block_number: Option<u128>,
    ) -> anyhow::Result<Self> {
        let mut indexer = TxPositionIndexer::new(db_path, reset_from)?;
        let stats_before_reset = indexer.get_stats()?;
        info!("indexer stats after load: {:?}", stats_before_reset);
        indexer
            .updates_by_segments(segment_dir, max_block_number)
            .await?;
        info!("indexer stats after updates: {:?}", indexer.get_stats()?);
        Ok(indexer)
    }

    pub(crate) fn get_tx_position(&self, tx_order: u64) -> anyhow::Result<Option<TxPosition>> {
        let rtxn = self.db_env.read_txn()?;
        let db = self.db;
        let ret = db.get(&rtxn, &tx_order)?;
        rtxn.commit()?;
        Ok(ret)
    }

    pub(crate) fn get_tx_positions_in_range(
        &self,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Vec<TxPosition>> {
        let rtxn = self.db_env.read_txn()?;
        let db = self.db;
        let mut tx_positions = Vec::new();
        let range = start..=end;
        let mut iter = db.range(&rtxn, &range)?;
        while let Some((_k, v)) = iter.next().transpose()? {
            tx_positions.push(v);
        }
        drop(iter);
        rtxn.commit()?;
        Ok(tx_positions)
    }

    fn create_env(db_path: PathBuf) -> anyhow::Result<Env> {
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAP_SIZE) // 16G
                .max_dbs(MAX_DBS)
                .open(db_path)?
        };
        Ok(env)
    }

    // init cursor by search last tx_order
    pub(crate) fn init_cursor(&mut self) -> anyhow::Result<()> {
        let rtxn = self.db_env.read_txn()?;
        let db = self.db;
        if let Some((k, v)) = db.last(&rtxn)? {
            self.last_tx_order = k;
            self.last_block_number = v.block_number;
        }
        rtxn.commit()?;
        Ok(())
    }

    fn reset_from(&self, from: u64) -> anyhow::Result<()> {
        let mut wtxn = self.db_env.write_txn()?;
        let db = self.db;

        let range = from..;
        let deleted_count = db.delete_range(&mut wtxn, &range)?;
        wtxn.commit()?;
        info!("deleted {} records from tx_order: {}", deleted_count, from);
        Ok(())
    }

    pub(crate) fn get_stats(&self) -> anyhow::Result<TxPositionIndexerStats> {
        let rtxn = self.db_env.read_txn()?;
        let db = self.db;
        let count = db.iter(&rtxn)?.count();
        rtxn.commit()?;
        Ok(TxPositionIndexerStats {
            total_tx_count: count as u64,
            last_tx_order: self.last_tx_order,
            last_block_number: self.last_block_number,
        })
    }

    pub(crate) async fn updates_by_segments(
        &mut self,
        segment_dir: Option<PathBuf>,
        max_block_number: Option<u128>,
    ) -> anyhow::Result<()> {
        let segment_dir = segment_dir.ok_or_else(|| anyhow!("segment_dir is required"))?;
        let ledger_tx_loader = LedgerTxGetter::new(segment_dir, true)?;
        let stop_at = if let Some(max_block_number) = max_block_number {
            min(max_block_number, ledger_tx_loader.get_max_chunk_id())
        } else {
            ledger_tx_loader.get_max_chunk_id()
        };

        if stop_at == 0 {
            info!("No segments found for tx position indexer, maybe in sync mode");
            return Ok(());
        }

        let mut block_number = self.last_block_number; // avoiding partial indexing
        let mut expected_tx_order = self.last_tx_order + 1;
        let mut done_block = 0;

        while block_number <= stop_at {
            let tx_list = ledger_tx_loader
                .load_ledger_tx_list(block_number, true, true)
                .await?;
            let tx_list = tx_list.unwrap();
            {
                let db = self.db;
                let mut wtxn = self.db_env.write_txn()?;
                for mut ledger_tx in tx_list {
                    let tx_order = ledger_tx.sequence_info.tx_order;
                    if tx_order < expected_tx_order {
                        continue;
                    }
                    if tx_order == self.last_tx_order + 1 {
                        info!(
                            "begin to index block: {}, tx_order: {}",
                            block_number, tx_order
                        );
                    }
                    if tx_order != expected_tx_order {
                        return Err(anyhow!(
                            "tx_order not continuous, expect: {}, got: {}",
                            expected_tx_order,
                            tx_order
                        ));
                    }
                    let tx_hash = ledger_tx.tx_hash();
                    let tx_position = TxPosition {
                        tx_order,
                        tx_hash,
                        block_number,
                    };
                    db.put(&mut wtxn, &tx_order, &tx_position)?;
                    expected_tx_order += 1;
                }
                wtxn.commit()?;
            }
            block_number += 1;
            done_block += 1;
            if done_block % 1000 == 0 {
                info!(
                    "done: block_cnt: {}; next_block_number: {}",
                    done_block, block_number
                );
            }
        }

        self.init_cursor()
    }

    pub(crate) fn close(&self) -> anyhow::Result<()> {
        let env = self.db_env.clone();
        env.force_sync()?;
        drop(env);
        Ok(())
    }
}

fn write_down_segments(
    chunk_id: u128,
    tx_order_start: u64,
    tx_order_end: u64,
    tx_list: &Vec<LedgerTransaction>,
    sequencer_keypair: &KanariKeyPair,
    segment_dir: PathBuf,
) -> anyhow::Result<()> {
    let batch = DABatch::new(
        chunk_id,
        tx_order_start,
        tx_order_end,
        tx_list,
        sequencer_keypair,
    )?;
    // ensure the batch is valid
    batch.verify(true)?;

    let segments = ChunkV0::from(batch).to_segments(DEFAULT_MAX_SEGMENT_SIZE);
    for segment in segments.iter() {
        let segment_path = segment_dir.join(segment.get_id().to_string());
        let mut writer = File::create(segment_path)?;
        writer.write_all(&segment.to_bytes())?;
        writer.flush()?;
    }
    Ok(())
}
