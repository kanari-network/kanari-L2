// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use kanari::KanariCli;
use std::process::exit;

#[cfg(not(target_env = "msvc"))]
mod allocator {
    use tikv_jemallocator::Jemalloc;

    pub type Allocator = Jemalloc;

    pub const fn allocator() -> Allocator {
        Jemalloc
    }
}

#[cfg(target_env = "msvc")]
mod allocator {
    use mimalloc::MiMalloc;

    pub type Allocator = MiMalloc;

    pub const fn allocator() -> Allocator {
        MiMalloc
    }
}

#[global_allocator]
static GLOBAL: allocator::Allocator = allocator::allocator();

/// kanari is a command line tools for Kanari Network
#[tokio::main]
async fn main() {
    let _ = tracing_subscriber::fmt::try_init();

    let opt = KanariCli::parse();
    let result = kanari::run_cli(opt).await;

    match result {
        Ok(s) => println!("{}", s),
        Err(e) => {
            println!("{}", e);
            exit(1);
        }
    }
}
