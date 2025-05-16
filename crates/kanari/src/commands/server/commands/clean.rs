// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use kanari_config::{BaseConfig, KanariOpt};
use kanari_types::error::{KanariError, KanariResult};
use std::{fs, io::Write, path::Path};
/// Clean the Kanari server storage
#[derive(Debug, Parser)]
pub struct CleanCommand {
    #[clap(flatten)]
    opt: KanariOpt,
    #[clap(long, short = 'f')]
    ///Force to clean without prompt
    force: bool,
}

#[allow(dead_code)]
impl CleanCommand {
    pub fn execute(self) -> KanariResult<()> {
        let base_config = BaseConfig::load_with_opt(&self.opt)?;
        let data_dir = base_config.data_dir();
        if !self.force {
            //prompt user to confirm
            let prompt = format!(
                "Are you sure to clean the kanari data dir: {:?} ?(Y/n)\n",
                data_dir
            );
            let mut input = String::new();
            std::io::stdout().write_all(prompt.as_bytes())?;
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() && input.trim().to_lowercase() != "y" {
                return Ok(());
            }
        }

        self.remove_store_dir(data_dir)?;
        println!("Kanari storage {:?} successfully cleaned", data_dir);

        Ok(())
    }

    fn remove_store_dir(&self, store_dir: &Path) -> KanariResult<()> {
        if !store_dir.exists() {
            return Ok(());
        }

        if !store_dir.is_dir() {
            return Err(KanariError::CleanServerError(format!(
                "{:?} is not a valid directory",
                store_dir
            )));
        }

        fs::remove_dir_all(store_dir).map_err(|e| KanariError::CleanServerError(e.to_string()))
    }
}
