// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::messages::ProposeBlock;
use crate::metrics::ProposerMetrics;
use crate::scc::StateCommitmentChain;
use async_trait::async_trait;
use coerce::actor::{context::ActorContext, message::Handler, Actor};
use moveos_store::MoveOSStore;
use prometheus::Registry;
use kanari_config::proposer_config::ProposerConfig;
use kanari_store::proposer_store::ProposerStore;
use kanari_store::KanariStore;
use kanari_types::crypto::KanariKeyPair;
use std::sync::Arc;

const PROPOSE_BLOCK_FN_NAME: &str = "propose_block";

pub struct ProposerActor {
    proposer_key: KanariKeyPair,
    scc: StateCommitmentChain,
    metrics: Arc<ProposerMetrics>,
}

impl ProposerActor {
    pub fn new(
        proposer_key: KanariKeyPair,
        moveos_store: MoveOSStore,
        kanari_store: KanariStore,
        registry: &Registry,
        config: ProposerConfig,
    ) -> anyhow::Result<Self> {
        let init_offset = config.init_offset;
        let last_proposed = kanari_store.get_last_proposed()?;
        // if init_offset is not None && init_offset - 1 > last_proposed, set last_proposed to init_offset - 1
        if let Some(init_offset) = init_offset {
            if let Some(last_proposed) = last_proposed {
                if init_offset - 1 > last_proposed {
                    kanari_store.set_last_proposed(init_offset - 1)?;
                }
            } else {
                kanari_store.set_last_proposed(init_offset - 1)?;
            }
        };

        let scc = StateCommitmentChain::new(kanari_store, moveos_store)?;

        Ok(Self {
            proposer_key,
            scc,
            metrics: Arc::new(ProposerMetrics::new(registry)),
        })
    }
}

impl Actor for ProposerActor {}

#[async_trait]
impl Handler<ProposeBlock> for ProposerActor {
    async fn handle(&mut self, _message: ProposeBlock, _ctx: &mut ActorContext) {
        let fn_name = PROPOSE_BLOCK_FN_NAME;
        let _timer = self
            .metrics
            .proposer_propose_block_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let block = self.scc.propose_block().await;
        match block {
            Ok(block) => {
                match block {
                    Some(block) => {
                        // TODO submit to the on-chain SCC contract use the proposer key
                        let _proposer_key = &self.proposer_key;
                        let ret = self.scc.set_last_proposed(block.block_number);
                        match ret {
                            Ok(_) => {
                                tracing::info!(
                                    "[ProposeBlock] done. block_number: {}",
                                    block.block_number,
                                );
                            }
                            Err(e) => {
                                tracing::error!("[ProposeBlock] set last proposed error: {:?}", e);
                            }
                        }

                        // TODO make new metric for matching real data submit to the chain
                        self.metrics
                            .proposer_propose_block_batch_size
                            .set(block.batch_size as i64);
                    }
                    None => {
                        tracing::debug!("[ProposeBlock] no transaction to propose block");
                    }
                };
            }
            Err(e) => {
                tracing::error!("[ProposeBlock] error: {:?}", e);
            }
        }
    }
}
