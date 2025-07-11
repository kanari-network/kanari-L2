// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;

use kanari_config::R_OPT_NET_HELP;
use kanari_indexer::IndexerStore;
use kanari_indexer::indexer_reader::IndexerReader;
use kanari_types::bitcoin::ord::Inscription;
use kanari_types::bitcoin::utxo::UTXO;
use kanari_types::error::KanariResult;
use kanari_types::indexer::state::{IndexerStateID, ObjectStateFilter, ObjectStateType};
use kanari_types::kanari_network::KanariChainID;
use moveos_types::state::MoveStructType;

use crate::commands::indexer::commands::init_indexer;

#[derive(Debug, Parser)]
pub struct BenchCommand {
    #[clap(long = "data-dir", short = 'd')]
    /// Path to data dir, this dir is base dir, the final data_dir is base_dir/chain_network_name
    pub base_data_dir: Option<PathBuf>,

    #[clap(long, help = "bench count", default_value = "10000")]
    pub count: Option<u64>,

    #[clap(long, help = "query filter: utxo/ord", default_value = "utxo")]
    pub query_filter: Option<String>,

    /// If local chainid, start the service with a temporary data store.
    /// All data would be deleted when the service is stopped.
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl BenchCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let (_, indexer_reader, _) = self.init();
        Self::count_cost_query(
            indexer_reader,
            self.query_filter.unwrap(),
            self.count.unwrap(),
        )
        .unwrap();
        Ok(())
    }

    fn init(&self) -> (IndexerStore, IndexerReader, Instant) {
        let start_time = Instant::now();
        let (indexer_store, indexer_reader) =
            init_indexer(self.base_data_dir.clone(), self.chain_id.clone()).unwrap();
        tracing::info!("indexer bench started");
        (indexer_store, indexer_reader, start_time)
    }

    fn count_cost_query(
        indexer_reader: IndexerReader,
        query_filter: String,
        count: u64,
    ) -> anyhow::Result<()> {
        let batch_size: u64 = count / 10;

        let (filter, state_type) = match query_filter.as_str() {
            "utxo" => (
                ObjectStateFilter::ObjectType(UTXO::struct_tag()),
                ObjectStateType::UTXO,
            ),
            "ord" => (
                ObjectStateFilter::ObjectType(Inscription::struct_tag()),
                ObjectStateType::Inscription,
            ),
            _ => (
                ObjectStateFilter::ObjectType(UTXO::struct_tag()),
                ObjectStateType::UTXO,
            ),
        };

        let start = Instant::now();
        let query_object_states = indexer_reader.query_object_ids_with_filter(
            filter.clone(),
            None,
            1,
            true,
            state_type.clone(),
        )?;
        let tx_order = query_object_states[0].1.tx_order;
        let mut state_index = query_object_states[0].1.state_index;
        tracing::info!(
            "bench start for tx_order: {}, state_index: {}. init query cost: {:?}",
            tx_order,
            state_index,
            start.elapsed()
        );
        state_index += 1;
        let mut total_duration: u128 = 0;
        let mut total_query: u64 = 0;
        loop {
            if total_query >= count {
                break;
            }
            let start = Instant::now();
            for _ in 0..batch_size {
                let query_object_states = indexer_reader.query_object_ids_with_filter(
                    filter.clone(),
                    Some(IndexerStateID::new(tx_order, state_index)),
                    1,
                    true,
                    state_type.clone(),
                )?;
                assert_eq!(query_object_states.len(), 1);
                total_query += 1;
                state_index += 1;
            }
            let duration = start.elapsed().as_millis();
            total_duration += duration;
            println!(
                "query: {}, avg_duration(ms): {:.2}",
                batch_size,
                total_duration as f64 / total_query as f64
            );
        }

        println!(
            "total query: {}, avg_duration(ms): {:.2},",
            total_query,
            total_duration as f64 / total_query as f64
        );

        Ok(())
    }
}
