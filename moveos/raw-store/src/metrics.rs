// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use metrics::metrics_util::LATENCY_SEC_BUCKETS;
use once_cell::sync::OnceCell;
use prometheus::{
    HistogramVec, IntCounterVec, IntGaugeVec, Registry, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry,
};
use std::sync::Arc;
use tap::TapFallible;

#[derive(Debug)]
pub struct RocksDBMetrics {
    pub rocksdb_total_sst_files_size: IntGaugeVec,
    pub rocksdb_total_blob_files_size: IntGaugeVec,
    pub rocksdb_size_all_mem_tables: IntGaugeVec,
    pub rocksdb_block_cache_capacity: IntGaugeVec,
    pub rocksdb_block_cache_usage: IntGaugeVec,
    pub rocksdb_block_cache_hit: IntGaugeVec,
    pub rocksdb_block_cache_miss: IntGaugeVec,
    pub rocksdb_mem_table_flush_pending: IntGaugeVec,
    pub rocskdb_compaction_pending: IntGaugeVec,
    pub rocskdb_num_running_compactions: IntGaugeVec,
    pub rocksdb_num_running_flushes: IntGaugeVec,
    pub rocskdb_background_errors: IntGaugeVec,
}

impl RocksDBMetrics {
    pub(crate) fn new(registry: &Registry) -> Self {
        RocksDBMetrics {
            rocksdb_total_sst_files_size: register_int_gauge_vec_with_registry!(
                "rocksdb_total_sst_files_size",
                "The storage size occupied by the sst files in the column family",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_total_blob_files_size: register_int_gauge_vec_with_registry!(
                "rocksdb_total_blob_files_size",
                "The storage size occupied by the blob files in the column family",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_size_all_mem_tables: register_int_gauge_vec_with_registry!(
                "rocksdb_size_all_mem_tables",
                "The memory size occupied by the column family's in-memory buffer",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_block_cache_capacity: register_int_gauge_vec_with_registry!(
                "rocksdb_block_cache_capacity",
                "The block cache capacity of the column family.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_block_cache_usage: register_int_gauge_vec_with_registry!(
                "rocksdb_block_cache_usage",
                "The memory size used by the column family in the block cache.",
                &["cf_name"],
                registry,
            )
            .unwrap(),

            rocksdb_block_cache_hit: register_int_gauge_vec_with_registry!(
                "rocksdb_block_cache_hit",
                "The cache hit counts by the column family in the block cache.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_block_cache_miss: register_int_gauge_vec_with_registry!(
                "rocksdb_block_cache_miss",
                "The cache miss counts by the column family in the block cache.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_mem_table_flush_pending: register_int_gauge_vec_with_registry!(
                "rocksdb_mem_table_flush_pending",
                "A 1 or 0 flag indicating whether a memtable flush is pending.
                If this number is 1, it means a memtable is waiting for being flushed,
                but there might be too many L0 files that prevents it from being flushed.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocskdb_compaction_pending: register_int_gauge_vec_with_registry!(
                "rocskdb_compaction_pending",
                "A 1 or 0 flag indicating whether a compaction job is pending.
                If this number is 1, it means some part of the column family requires
                compaction in order to maintain shape of LSM tree, but the compaction
                is pending because the desired compaction job is either waiting for
                other dependent compactions to be finished or waiting for an available
                compaction thread.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocskdb_num_running_compactions: register_int_gauge_vec_with_registry!(
                "rocskdb_num_running_compactions",
                "The number of compactions that are currently running for the column family.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocksdb_num_running_flushes: register_int_gauge_vec_with_registry!(
                "rocksdb_num_running_flushes",
                "The number of flushes that are currently running for the column family.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            rocskdb_background_errors: register_int_gauge_vec_with_registry!(
                "rocskdb_background_errors",
                "The accumulated number of RocksDB background errors.",
                &["cf_name"],
                registry,
            )
            .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct RawStoreMetrics {
    pub raw_store_iter_latency_seconds: HistogramVec,
    pub raw_store_iter_bytes: HistogramVec,
    pub raw_store_iter_keys: HistogramVec,
    pub raw_store_get_latency_seconds: HistogramVec,
    pub raw_store_get_bytes: HistogramVec,
    pub raw_store_multiget_latency_seconds: HistogramVec,
    pub raw_store_multiget_bytes: HistogramVec,
    pub raw_store_put_latency_seconds: HistogramVec,
    pub raw_store_put_bytes: HistogramVec,
    pub raw_store_write_batch_latency_seconds: HistogramVec,
    pub raw_store_write_batch_bytes: HistogramVec,
    pub raw_store_put_sync_latency_seconds: HistogramVec,
    pub raw_store_put_sync_bytes: HistogramVec,
    pub raw_store_write_batch_sync_latency_seconds: HistogramVec,
    pub raw_store_write_batch_sync_bytes: HistogramVec,
    pub raw_store_delete_latency_seconds: HistogramVec,
    pub raw_store_deletes: IntCounterVec,
    pub raw_store_num_active_cf_handles: IntGaugeVec,
}

impl RawStoreMetrics {
    pub(crate) fn new(registry: &Registry) -> Self {
        RawStoreMetrics {
            raw_store_iter_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_iter_latency_seconds",
                "Raw store iter latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_iter_bytes: register_histogram_vec_with_registry!(
                "raw_store_iter_bytes",
                "Raw store iter size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_iter_keys: register_histogram_vec_with_registry!(
                "raw_store_iter_keys",
                "Raw store iter num keys",
                &["cf_name"],
                registry,
            )
            .unwrap(),
            raw_store_get_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_get_latency_seconds",
                "Raw store get latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_get_bytes: register_histogram_vec_with_registry!(
                "raw_store_get_bytes",
                "Raw store get call returned data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry
            )
            .unwrap(),
            raw_store_multiget_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_multiget_latency_seconds",
                "Raw store multiget latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_multiget_bytes: register_histogram_vec_with_registry!(
                "raw_store_multiget_bytes",
                "Raw store multiget call returned data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_put_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_put_latency_seconds",
                "Raw store put latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_put_bytes: register_histogram_vec_with_registry!(
                "raw_store_put_bytes",
                "Raw store put call puts data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_write_batch_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_write_batch_latency_seconds",
                "Raw store write batch latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_write_batch_bytes: register_histogram_vec_with_registry!(
                "raw_store_write_batch_bytes",
                "Raw store write batch puts data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_put_sync_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_put_sync_latency_seconds",
                "Raw store put sync latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_put_sync_bytes: register_histogram_vec_with_registry!(
                "raw_store_put_sync_bytes",
                "Raw store put sync call puts data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_write_batch_sync_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_write_batch_sync_latency_seconds",
                "Raw store write batch sync latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_write_batch_sync_bytes: register_histogram_vec_with_registry!(
                "raw_store_write_batch_sync_bytes",
                "Raw store write batch sync call puts data size in bytes",
                &["cf_name"],
                prometheus::exponential_buckets(1.0, 4.0, 15)
                    .unwrap()
                    .to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_delete_latency_seconds: register_histogram_vec_with_registry!(
                "raw_store_delete_latency_seconds",
                "Raw store delete latency in seconds",
                &["cf_name"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            raw_store_deletes: register_int_counter_vec_with_registry!(
                "raw_store_deletes",
                "Raw store delete calls",
                &["cf_name"],
                registry
            )
            .unwrap(),
            raw_store_num_active_cf_handles: register_int_gauge_vec_with_registry!(
                "raw_store_num_active_cf_handles",
                "Number of active column family handles",
                &["cf_name"],
                registry,
            )
            .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct DBMetrics {
    pub raw_store_metrics: RawStoreMetrics,
    pub rocksdb_metrics: RocksDBMetrics,
}

static DB_METRICS_ONCE: OnceCell<Arc<DBMetrics>> = OnceCell::new();

impl DBMetrics {
    pub fn new(registry: &Registry) -> Self {
        DBMetrics {
            raw_store_metrics: RawStoreMetrics::new(registry),
            rocksdb_metrics: RocksDBMetrics::new(registry),
        }
    }

    pub fn init(registry: &Registry) -> &'static Arc<DBMetrics> {
        // Initialize this before creating any instance of StoreInstance
        // only ever initialize db metrics once with a registry whereas
        // in the code we might want to initialize it with different
        // registries.
        let _ = DB_METRICS_ONCE
            .set(Self::inner_init(registry))
            .tap_err(|_| tracing::warn!("DBMetrics registry overwritten"));
        DB_METRICS_ONCE.get().unwrap()
    }

    fn inner_init(registry: &Registry) -> Arc<DBMetrics> {
        Arc::new(DBMetrics::new(registry))
    }

    pub fn increment_num_active_dbs(&self, cf_name: &str) {
        self.raw_store_metrics
            .raw_store_num_active_cf_handles
            .with_label_values(&[cf_name])
            .inc();
    }

    pub fn decrement_num_active_dbs(&self, cf_name: &str) {
        self.raw_store_metrics
            .raw_store_num_active_cf_handles
            .with_label_values(&[cf_name])
            .dec();
    }

    pub fn get() -> Option<&'static Arc<DBMetrics>> {
        DB_METRICS_ONCE.get()
    }

    pub fn get_or_init(registry: &Registry) -> &'static Arc<DBMetrics> {
        DB_METRICS_ONCE.get_or_init(|| Self::inner_init(registry).clone())
    }
}
