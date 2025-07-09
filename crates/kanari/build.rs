// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use vergen_git2::{BuildBuilder, CargoBuilder, Emitter, Git2Builder, RustcBuilder};

fn main() -> Result<()> {
    let result = Emitter::default()
        .add_instructions(&BuildBuilder::all_build()?)?
        .add_instructions(&CargoBuilder::all_cargo()?)?
        .add_instructions(&Git2Builder::all_git()?)?
        .add_instructions(&RustcBuilder::all_rustc()?)?
        .emit();

    match result {
        Ok(_) => println!("cargo:warning=Build information generated successfully"),
        Err(e) => {
            println!("cargo:warning=Failed to generate build information: {}", e);
            // Continue build even if version info fails
        }
    }

    Ok(())
}
