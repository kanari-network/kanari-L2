// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use fastcrypto::traits::EncodeDecodeBase64;
use kanari_types::crypto::KanariKeyPair;

/// Write Base64 encoded `flag || privkey` to file.
pub fn write_keypair_to_file<P: AsRef<std::path::Path>>(
    keypair: &KanariKeyPair,
    path: P,
) -> anyhow::Result<()> {
    let contents = keypair.encode_base64();
    std::fs::write(path, contents)?;
    Ok(())
}

/// Read from file as Base64 encoded `flag || privkey` and return a KanariKeyPair.
pub fn read_keypair_from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<KanariKeyPair> {
    let contents = std::fs::read_to_string(path)?;
    KanariKeyPair::decode_base64(contents.as_str().trim()).map_err(|e| anyhow!(e))
}
