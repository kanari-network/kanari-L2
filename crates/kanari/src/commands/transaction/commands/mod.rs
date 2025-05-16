// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use moveos_types::h256::H256;
use kanari_types::transaction::{
    kanari::PartiallySignedKanariTransaction, KanariTransaction, KanariTransactionData,
};
use serde::{Deserialize, Serialize};
use std::{env, fs::File, io::Write, path::PathBuf};

pub mod build;
pub mod get_transactions_by_hash;
pub mod get_transactions_by_order;
pub mod query;
pub mod sign;
pub mod sign_order;
pub mod submit;

pub(crate) enum FileOutputData {
    KanariTransactionData(KanariTransactionData),
    SignedKanariTransaction(KanariTransaction),
    PartiallySignedKanariTransaction(PartiallySignedKanariTransaction),
}

impl FileOutputData {
    pub fn tx_hash(&self) -> H256 {
        match self {
            FileOutputData::KanariTransactionData(data) => data.tx_hash(),
            FileOutputData::SignedKanariTransaction(data) => data.data.tx_hash(),
            FileOutputData::PartiallySignedKanariTransaction(data) => data.data.tx_hash(),
        }
    }

    pub fn file_signatory_suffix(&self) -> String {
        match self {
            FileOutputData::KanariTransactionData(data) => data.sender.to_bech32(),
            FileOutputData::SignedKanariTransaction(data) => data.sender().to_bech32(),
            FileOutputData::PartiallySignedKanariTransaction(data) => data.signatories().to_string(),
        }
    }

    pub fn file_suffix(&self) -> &str {
        match self {
            FileOutputData::KanariTransactionData(_) => "ktd",
            FileOutputData::SignedKanariTransaction(_) => "skt",
            FileOutputData::PartiallySignedKanariTransaction(_) => "pskt",
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            FileOutputData::KanariTransactionData(data) => data.encode(),
            FileOutputData::SignedKanariTransaction(data) => data.encode(),
            FileOutputData::PartiallySignedKanariTransaction(data) => data.encode(),
        }
    }

    pub fn default_output_file_path(&self) -> Result<PathBuf> {
        let temp_dir = env::temp_dir();
        let tx_hash = self.tx_hash();
        let file_name = format!(
            "{}.{}.{}",
            hex::encode(&tx_hash[..8]),
            self.file_signatory_suffix(),
            self.file_suffix()
        );
        Ok(temp_dir.join(file_name))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FileOutput {
    pub content: String,
    pub path: String,
}

impl FileOutput {
    pub fn write_to_file(data: FileOutputData, output_path: Option<String>) -> Result<Self> {
        let path = match output_path {
            Some(path) => PathBuf::from(path),
            None => data.default_output_file_path()?,
        };
        let mut file = File::create(&path)?;
        // we write the hex encoded data to the file
        // not the binary data, for better readability
        let hex = hex::encode(data.encode());
        file.write_all(hex.as_bytes())?;
        Ok(FileOutput {
            content: hex,
            path: path.to_string_lossy().to_string(),
        })
    }
}
