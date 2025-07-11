// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use serde::Serialize;

use kanari_types::transaction::L1Block;

pub struct BcsSerializeSizeFunContainer<T: ?Sized + Serialize> {
    pub func: fn(&T) -> u64,
    pub name: &'static str,
}

pub fn serialized_size<T>(value: &T) -> u64
where
    T: ?Sized + Serialize,
{
    bcs::serialized_size(value).unwrap() as u64
}

pub fn encode_size<T>(value: &T) -> u64
where
    T: ?Sized + Serialize,
{
    bcs::to_bytes(value)
        .expect("encode transaction should success")
        .len() as u64
}

pub fn bcs_serialized_size_benchmark(c: &mut Criterion) {
    let l1_block = L1Block::default();
    let mut group = c.benchmark_group("bcs_serialized_size_bench");

    let funcs = [
        BcsSerializeSizeFunContainer {
            func: serialized_size,
            name: "serialized_size",
        },
        BcsSerializeSizeFunContainer {
            func: encode_size,
            name: "encode_size",
        },
    ];
    for func in funcs.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(func.name),
            &func.func,
            |b, &func| {
                b.iter(|| {
                    let _ = func(&l1_block);
                })
            },
        );
    }
    group.finish();
}

pub fn anyhow_error_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("anyhow_error_bench");
    group.bench_function("anyhow_v1.0.93", |b| {
        b.iter(|| {
            let _ = anyhow_new::anyhow!("anyhow error");
        })
    });
    group.bench_function("anyhow_v1.0.76", |b| {
        b.iter(|| {
            let _ = anyhow_old::anyhow!("anyhow error");
        })
    });
    group.finish();
}

criterion_group! {
    benchs, bcs_serialized_size_benchmark, anyhow_error_benchmark
}

criterion_main!(benchs);
