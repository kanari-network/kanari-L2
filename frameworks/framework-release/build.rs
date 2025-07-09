// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use std::env::current_dir;

fn main() {
    if std::env::var("SKIP_STDLIB_BUILD").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "WARN");
        }
        let _ = tracing_subscriber::fmt::try_init();
        let current_dir = current_dir().expect("Should be able to get current dir");
        // Get the project root directory
        let mut root_dir = current_dir;
        root_dir.pop();
        root_dir.pop();

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/move-stdlib")
                .join("Move.toml")
                .display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/move-stdlib")
                .join("sources")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/moveos-stdlib")
                .join("Move.toml")
                .display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/moveos-stdlib")
                .join("sources")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/kanari-framework")
                .join("Move.toml")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/kanari-framework")
                .join("sources")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/bitcoin-move")
                .join("Move.toml")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/bitcoin-move")
                .join("sources")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/kanari-nursery")
                .join("Move.toml")
                .display()
        );

        println!(
            "cargo:rerun-if-changed={}",
            root_dir
                .join("frameworks/kanari-nursery")
                .join("sources")
                .display()
        );

        // Try to release the framework with better error handling
        println!("cargo:warning=Building framework release...");
        match framework_builder::releaser::release_latest() {
            Ok(msgs) => {
                for msg in msgs {
                    println!("cargo::warning=\"{}\"", msg);
                }
                println!("cargo:warning=Framework release completed successfully");
            }
            Err(e) => {
                println!(
                    "cargo::warning=\"Failed to release latest framework: {:?}\"",
                    e
                );
                // On Windows, if we get a stack overflow or similar error,
                // we should still try to continue the build process
                if cfg!(windows) {
                    println!(
                        "cargo:warning=Framework build failed on Windows, this may cause runtime issues"
                    );
                    println!(
                        "cargo:warning=Consider building on a non-Windows platform for full functionality"
                    );
                }
            }
        }
    }
}
