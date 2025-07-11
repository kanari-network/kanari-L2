// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AccumulatorNode;
use crate::node_index::NodeIndex;
use anyhow::Result;
use moveos_types::h256::H256;
use std::any::type_name;

pub mod mock;
pub mod rocks;

pub trait AccumulatorTreeStore: std::marker::Send + std::marker::Sync {
    fn store_type(&self) -> &'static str {
        type_name::<Self>()
    }

    ///get node by node hash
    fn get_node(&self, hash: H256) -> Result<Option<AccumulatorNode>>;
    /// multiple get nodes
    fn multiple_get(&self, hash_vec: Vec<H256>) -> Result<Vec<Option<AccumulatorNode>>>;

    /// save node
    fn save_node(&self, node: AccumulatorNode) -> Result<()>;
    /// batch save nodes
    fn save_nodes(&self, nodes: Vec<AccumulatorNode>) -> Result<()>;
    ///delete node
    fn delete_nodes(&self, node_hash_vec: Vec<H256>) -> Result<()>;
}

pub type NodeCacheKey = NodeIndex;
