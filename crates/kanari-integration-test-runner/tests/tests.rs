// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use kanari_integration_test_runner::run_test;
use std::path::Path;
use tokio::runtime::Runtime;

pub fn async_run_test(path: &Path) -> Result<(), Box<dyn std::error::Error + 'static>> {
    let runtime =
        Runtime::new().expect("Failed to create Tokio runtime when execute async run test ");
    runtime.block_on(async { run_test(path) })
}

datatest_stable::harness! {
    { test = async_run_test, root = "tests", pattern = r".*\.(mvir|move)$" },
}
