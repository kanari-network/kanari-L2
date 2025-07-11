// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use criterion::{Criterion, criterion_group, criterion_main};
use kanari_benchmarks::config::{BenchTxConfig, configure_criterion};
use kanari_benchmarks::tx::{create_l2_tx, gen_sequencer};
use kanari_framework_tests::binding_test;
use kanari_test_transaction_builder::TestTransactionBuilder;
use kanari_types::transaction::LedgerTxData;
use std::time::Duration;

pub fn tx_sequence_benchmark(c: &mut Criterion) {
    let config = BenchTxConfig::load();

    let binding_test = binding_test::RustBindingTest::new_in_tokio().unwrap();
    let kanari_key_pair = binding_test.sequencer_kp().copy();

    let sequencer_keypair = kanari_key_pair.copy();
    let mut sequencer = gen_sequencer(
        sequencer_keypair,
        binding_test.executor().get_kanari_store(),
        &binding_test.registry_service.default_registry(),
    )
    .unwrap();

    let tx_type = config.tx_type.unwrap().clone();

    let mut test_transaction_builder = TestTransactionBuilder::new(kanari_key_pair.copy());
    let tx_cnt = 600;
    let transactions: Vec<_> = (0..tx_cnt)
        .map(|n| {
            let tx = create_l2_tx(&mut test_transaction_builder, n, tx_type.clone()).unwrap();
            LedgerTxData::L2Tx(tx.clone())
        })
        .collect();
    let mut transactions_iter = transactions.into_iter().cycle();

    c.bench_function("tx_sequence", |b| {
        b.iter(|| {
            let tx = transactions_iter.next().unwrap();
            sequencer.sequence(tx.clone()).unwrap()
        });
    });
}

criterion_group! {
    name = tx_sequence_bench;
    config = configure_criterion(None).measurement_time(Duration::from_millis(200));
    targets = tx_sequence_benchmark
}

criterion_main!(tx_sequence_bench);
