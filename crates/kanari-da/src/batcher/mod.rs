// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use kanari_config::settings::KANARI_BATCH_INTERVAL;
use kanari_store::KanariStore;
use kanari_store::da_store::DAMetaStore;

struct InProgressBatch {
    tx_order_start: u64,
    tx_order_end: u64,
    start_timestamp: u64,
}

impl InProgressBatch {
    fn init() -> Self {
        Self {
            tx_order_start: 0,
            tx_order_end: 0,
            start_timestamp: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::init();
    }

    // create a new batch with the first transaction
    fn begin_with(&mut self, tx_order: u64, mut tx_timestamp: u64) {
        if tx_timestamp == 0 {
            tx_timestamp = 1;
            tracing::warn!("tx_timestamp is 0, should not happen, set to 1");
        }

        self.tx_order_start = tx_order;
        self.tx_order_end = tx_order;
        self.start_timestamp = tx_timestamp;
    }

    // Append transaction to batch:
    // 1. If the batch is empty(batch_start_time is 0), reset for making a new batch
    // 2. If the batch is not empty, check if the transaction is in the interval:
    //  1. If the transaction is in the interval, update tx_order_end
    //  2. If the transaction is not in the interval, return tx range and wait for reset
    fn append_transaction(&mut self, tx_order: u64, tx_timestamp: u64) -> Option<(u64, u64)> {
        if self.start_timestamp == 0 {
            self.begin_with(tx_order, tx_timestamp);
            return None;
        }

        let last_tx_order_end = self.tx_order_end;
        if tx_order != last_tx_order_end + 1 {
            tracing::error!(
                "failed to make new batch: transaction order is not continuous, last: {}, current: {}",
                last_tx_order_end,
                tx_order
            );
            return None;
        }

        self.tx_order_end = tx_order;

        if tx_timestamp < self.start_timestamp ||        // backwards checking first, avoid overflow
            tx_timestamp - self.start_timestamp < KANARI_BATCH_INTERVAL
        {
            return None;
        }

        Some((self.tx_order_start, self.tx_order_end))
    }
}

pub struct BatchMaker {
    pending_tx: PendingTx,
    in_progress_batch: InProgressBatch,
    kanari_store: KanariStore,
}

struct PendingTx {
    tx_order: u64,
    tx_timestamp: u64,
}

impl PendingTx {
    fn new() -> Self {
        Self {
            tx_order: 0,
            tx_timestamp: 0,
        }
    }

    fn revert(&mut self, tx_order: u64) -> anyhow::Result<()> {
        let pending_tx_order = self.tx_order;
        if tx_order != pending_tx_order {
            return Err(anyhow!(
                "failed to revert pending transaction: transaction order is not continuous, pending_tx_order: {}, revert_tx_order: {}",
                pending_tx_order,
                tx_order
            ));
        }
        self.tx_order = 0;
        self.tx_timestamp = 0;
        Ok(())
    }

    fn push(&mut self, tx_order: u64, tx_timestamp: u64) -> Option<PendingTx> {
        let old = if self.tx_order == 0 {
            None
        } else {
            Some(PendingTx {
                tx_order: self.tx_order,
                tx_timestamp: self.tx_timestamp,
            })
        };
        self.tx_order = tx_order;
        self.tx_timestamp = tx_timestamp;
        old
    }
}

impl BatchMaker {
    pub fn new(kanari_store: KanariStore) -> Self {
        Self {
            pending_tx: PendingTx::new(),
            in_progress_batch: InProgressBatch::init(),
            kanari_store,
        }
    }

    // append transaction:
    // 1. push the new transaction to pending_tx return the old one if it has
    // 2. add the old transaction to the batch, return block number if a new batch is made
    pub fn append_transaction(&mut self, tx_order: u64, tx_timestamp: u64) -> Option<u128> {
        if let Some(old) = self.pending_tx.push(tx_order, tx_timestamp) {
            if let Some(block_number) = self.add_to_batch(old.tx_order, old.tx_timestamp) {
                return Some(block_number);
            }
        }
        None
    }

    // revert pending transaction
    pub fn revert_transaction(&mut self, tx_order: u64) -> anyhow::Result<()> {
        self.pending_tx.revert(tx_order)
    }

    // add transaction to the batch, return block number if a new batch is made
    fn add_to_batch(&mut self, tx_order: u64, tx_timestamp: u64) -> Option<u128> {
        let order_range = self
            .in_progress_batch
            .append_transaction(tx_order, tx_timestamp);
        if let Some((tx_order_start, tx_order_end)) = order_range {
            match self
                .kanari_store
                .append_submitting_block(tx_order_start, tx_order_end)
            {
                Ok(block_number) => {
                    // Successfully appended, return the block number & reset the batch
                    self.in_progress_batch.reset();
                    return Some(block_number);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to append submitting block for range ({}, {}): {}",
                        tx_order_start,
                        tx_order_end,
                        e
                    );
                }
            }
        };
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_in_progress_batch() {
        let mut in_progress_batch = InProgressBatch::init();
        assert_eq!(in_progress_batch.append_transaction(1, 1), None);

        assert_eq!(in_progress_batch.append_transaction(2, 2), None);

        assert_eq!(in_progress_batch.append_transaction(3, 3), None);

        assert_eq!(in_progress_batch.append_transaction(4, 4), None);

        assert_eq!(
            in_progress_batch.append_transaction(5, 1 + KANARI_BATCH_INTERVAL),
            Some((1, 5))
        );

        assert_eq!(in_progress_batch.append_transaction(6, 6), None);

        assert_eq!(in_progress_batch.append_transaction(7, 7), None);

        assert_eq!(in_progress_batch.append_transaction(8, 8), None);

        assert_eq!(
            in_progress_batch.append_transaction(9, 6 + KANARI_BATCH_INTERVAL),
            Some((1, 9))
        );

        in_progress_batch.reset();

        assert_eq!(in_progress_batch.append_transaction(6, 6), None);

        assert_eq!(in_progress_batch.append_transaction(7, 7), None);

        assert_eq!(in_progress_batch.append_transaction(8, 8), None);

        assert_eq!(
            in_progress_batch.append_transaction(9, 6 + KANARI_BATCH_INTERVAL),
            Some((6, 9))
        );
    }
}
