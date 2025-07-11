// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0s

use crate::node_index::FrozenSubTreeIterator;
use crate::node_index::{MAX_ACCUMULATOR_PROOF_DEPTH, NodeIndex};
use crate::tree_store::NodeCacheKey;
use crate::{AccumulatorNode, AccumulatorTreeStore, LeafCount, MAX_CACHE_SIZE, NodeCount};
use anyhow::{Result, bail, format_err};
use lru::LruCache;
use mirai_annotations::*;
use moveos_types::h256::{ACCUMULATOR_PLACEHOLDER_HASH, H256};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

pub struct AccumulatorTree {
    /// frozen subtree roots hashes.
    frozen_subtree_roots: Vec<H256>,
    /// The total number of leaves in this accumulator.
    pub(crate) num_leaves: LeafCount,
    /// The total number of nodes in this accumulator.
    pub(crate) num_nodes: NodeCount,
    /// The root hash of this accumulator.
    pub(crate) root_hash: H256,
    /// The index cache
    index_cache: LruCache<NodeCacheKey, H256>,
    /// The storage of accumulator.
    pub(crate) store: Arc<dyn AccumulatorTreeStore>,
    /// The temp update nodes
    update_nodes: HashMap<H256, AccumulatorNode>,
    /// index of frozen_subtree_roots
    /// max size will be 64
    index_frozen_subtrees: HashMap<NodeIndex, H256>,
    /// index_to_freeze NodeIndex => H256
    /// less than max_to_freeze
    index_to_freeze: HashMap<NodeIndex, H256>,
}

impl AccumulatorTree {
    pub fn new(
        frozen_subtree_roots: Vec<H256>,
        num_leaves: LeafCount,
        num_nodes: NodeCount,
        root_hash: H256,
        store: Arc<dyn AccumulatorTreeStore>,
    ) -> Self {
        let frozen_subtrees = if !frozen_subtree_roots.is_empty() {
            FrozenSubTreeIterator::new(num_leaves)
                .zip(frozen_subtree_roots.clone())
                .collect()
        } else {
            HashMap::new()
        };

        Self {
            frozen_subtree_roots,
            index_cache: LruCache::new(NonZeroUsize::new(MAX_CACHE_SIZE).unwrap()),
            num_leaves,
            num_nodes,
            root_hash,
            store,
            update_nodes: HashMap::new(),
            index_frozen_subtrees: frozen_subtrees,
            index_to_freeze: HashMap::new(),
        }
    }

    pub fn new_empty(store: Arc<dyn AccumulatorTreeStore>) -> Self {
        Self::new(vec![], 0, 0, *ACCUMULATOR_PLACEHOLDER_HASH, store)
    }

    ///append from multiple leaves
    pub fn append(&mut self, new_leaves: &[H256]) -> Result<H256> {
        // Deal with the case where new_leaves is empty
        if new_leaves.is_empty() {
            return if self.num_leaves == 0 {
                Ok(*ACCUMULATOR_PLACEHOLDER_HASH)
            } else {
                Ok(self.root_hash)
            };
        }
        let num_new_leaves = new_leaves.len();
        let last_new_leaf_count = self.num_leaves + num_new_leaves as LeafCount;
        let mut new_num_nodes = self.num_nodes;
        let root_level = NodeIndex::root_level_from_leaf_count(last_new_leaf_count);
        let mut to_freeze = Vec::with_capacity(Self::max_to_freeze(num_new_leaves, root_level));
        // Iterate over the new leaves, adding them to to_freeze and then adding any frozen parents
        // when right children are encountered.  This has the effect of creating frozen nodes in
        // perfect post-order, which can be used as a strictly increasing append only index for
        // the underlying storage.
        //
        // We will track newly created left siblings while iterating so we can pair them with their
        // right sibling, if and when it becomes frozen.  If the frozen left sibling is not created
        // in this iteration, it must already exist in storage.
        let mut left_siblings: Vec<(_, _)> = Vec::new();
        for (leaf_offset, leaf) in new_leaves.iter().enumerate() {
            let leaf_pos = NodeIndex::from_leaf_index(self.num_leaves + leaf_offset as LeafCount);
            let mut hash = *leaf;
            to_freeze.push(AccumulatorNode::new_leaf(leaf_pos, hash));
            self.index_to_freeze.insert(leaf_pos, hash);

            new_num_nodes += 1;
            let mut pos = leaf_pos;
            while pos.is_right_child() {
                // let mut internal_node = AccumulatorNode::Empty;
                let sibling = pos.sibling();

                let internal_node = match left_siblings.pop() {
                    Some((x, left_hash)) => {
                        assert_eq!(x, sibling);
                        AccumulatorNode::new_internal(pos.parent(), left_hash, hash)
                    }
                    None => AccumulatorNode::new_internal(
                        pos.parent(),
                        // because left must be frozen and it must be in index_frozen_subtrees or index_to_frozen
                        self.get_node_frozen_hash(sibling)?
                            .unwrap_or(*ACCUMULATOR_PLACEHOLDER_HASH),
                        hash,
                    ),
                };
                hash = internal_node.hash();
                pos = pos.parent();
                to_freeze.push(internal_node.clone());
                self.index_to_freeze.insert(internal_node.index(), hash);

                new_num_nodes += 1;
            }
            // The node remaining must be a left child, possibly the root of a complete binary tree.
            left_siblings.push((pos, hash));
        }

        let mut not_frozen_nodes = vec![];
        // Now reconstruct the final root hash by walking up to root level and adding
        // placeholder hash nodes as needed on the right, and left siblings that have either
        // been newly created or read from storage.
        let (mut pos, mut hash) = left_siblings.pop().expect("Must have at least one node");
        for _ in pos.level()..root_level {
            hash = if pos.is_left_child() {
                let not_frozen = AccumulatorNode::new_internal(
                    pos.parent(),
                    hash,
                    *ACCUMULATOR_PLACEHOLDER_HASH,
                );
                not_frozen_nodes.push(not_frozen.clone());
                not_frozen.hash()
            } else {
                let sibling = pos.sibling();
                match left_siblings.pop() {
                    Some((x, left_hash)) => {
                        assert_eq!(x, sibling);
                        let not_frozen =
                            AccumulatorNode::new_internal(pos.parent(), left_hash, hash);
                        not_frozen_nodes.push(not_frozen.clone());
                        not_frozen.hash()
                    }
                    None => {
                        let not_frozen = AccumulatorNode::new_internal(
                            pos.parent(),
                            self.get_node_frozen_hash(sibling)?
                                .unwrap_or(*ACCUMULATOR_PLACEHOLDER_HASH),
                            hash,
                        );
                        not_frozen_nodes.push(not_frozen.clone());
                        not_frozen.hash()
                    }
                }
            };
            pos = pos.parent();
        }

        debug_assert!(left_siblings.is_empty());
        //update frozen tag
        to_freeze = to_freeze
            .iter()
            .map(|node| {
                node.clone().frozen().expect("frozen must have value");
                node.clone()
            })
            .collect();
        //aggregator all nodes
        not_frozen_nodes.extend_from_slice(&to_freeze);
        self.update_temp_nodes(not_frozen_nodes.clone());
        // update to cache
        self.update_cache(not_frozen_nodes);
        // update self properties
        self.root_hash = hash;
        self.num_leaves = last_new_leaf_count;
        self.frozen_subtree_roots = self.scan_frozen_subtree_roots()?;
        self.index_frozen_subtrees = FrozenSubTreeIterator::new(self.num_leaves)
            .zip(self.frozen_subtree_roots.clone())
            .collect();
        self.index_to_freeze.clear();
        self.num_nodes = new_num_nodes;

        Ok(hash)
    }

    /// Get node from store
    fn get_node(&self, hash: H256) -> Result<Option<AccumulatorNode>> {
        let updates = &self.update_nodes;
        if !updates.is_empty() {
            if let Some(node) = updates.get(&hash) {
                return Ok(Some(node.clone()));
            }
        }
        self.store.get_node(hash)
    }

    /// Flush node to storage
    pub fn flush(&mut self) -> Result<()> {
        let nodes = &mut self.update_nodes;
        if !nodes.is_empty() {
            let nodes_vec = nodes
                .iter()
                .map(|(_, node)| node.clone())
                .collect::<Vec<AccumulatorNode>>();

            self.store.save_nodes(nodes_vec)?;
            nodes.clear();
        }
        Ok(())
    }

    /// Pop unsaved nodes for atomic updates with other operations.
    pub fn pop_unsaved_nodes(&mut self) -> Option<Vec<AccumulatorNode>> {
        let nodes = &mut self.update_nodes;
        if !nodes.is_empty() {
            let nodes_vec = nodes
                .iter()
                .map(|(_, node)| node.clone())
                .collect::<Vec<AccumulatorNode>>();
            return Some(nodes_vec);
        }

        None
    }

    pub fn clear_after_save(&mut self) {
        self.update_nodes.clear();
    }

    fn scan_frozen_subtree_roots(&mut self) -> Result<Vec<H256>> {
        FrozenSubTreeIterator::new(self.num_leaves)
            .map(|p| {
                self.get_node_frozen_hash(p)?
                    .ok_or_else(|| format_err!("frozen root {:?} must have value, but get none", p))
            })
            .collect()
    }

    pub fn get_frozen_subtree_roots(&self) -> Vec<H256> {
        self.frozen_subtree_roots.clone()
    }

    /// filter function can be applied to filter out certain siblings.
    pub(crate) fn get_siblings(
        &mut self,
        leaf_index: u64,
        filter: impl Fn(NodeIndex) -> bool,
    ) -> Result<Vec<H256>> {
        let root_pos = NodeIndex::root_from_leaf_count(self.num_leaves);
        let siblings = NodeIndex::from_leaf_index(leaf_index)
            .iter_ancestor_sibling()
            .take(root_pos.level() as usize)
            .filter_map(|p| {
                if filter(p) {
                    Some(self.get_node_hash_always(p))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(siblings)
    }

    /// Get node hash by index.
    pub(crate) fn get_node_hash(&mut self, node_index: NodeIndex) -> Result<Option<H256>> {
        let idx = self.rightmost_leaf_index();
        if node_index.is_placeholder(idx) {
            Ok(None)
        } else {
            Ok(Some(self.get_node_hash_always(node_index)?))
        }
    }

    fn get_node_frozen_hash(&mut self, node_index: NodeIndex) -> Result<Option<H256>> {
        let value = self.index_frozen_subtrees.get(&node_index);
        if value.is_some() {
            return Ok(value.cloned());
        }
        Ok(self.index_to_freeze.get(&node_index).cloned())
    }

    /// Update node to cache.
    fn update_cache(&mut self, node_vec: Vec<AccumulatorNode>) {
        self.save_node_indexes(node_vec)
    }

    fn update_temp_nodes(&mut self, nodes: Vec<AccumulatorNode>) {
        for node in nodes {
            self.update_nodes.insert(node.hash(), node);
        }
    }

    fn get_node_index(&mut self, key: NodeCacheKey) -> Option<H256> {
        self.index_cache.get(&key).copied()
    }

    /// Get node hash always.
    fn get_node_hash_always(&mut self, index: NodeIndex) -> Result<H256> {
        // get hash from cache
        let mut temp_index = index;
        let mut index_key = temp_index;
        if let Some(node_hash) = self.index_frozen_subtrees.get(&index_key) {
            return Ok(*node_hash);
        }
        if let Some(node_hash) = self.get_node_index(index_key) {
            return Ok(node_hash);
        }
        // find parent hash,then get node by parent hash
        let root_index = NodeIndex::root_from_leaf_count(self.num_leaves);
        let level = root_index.level() + 1;
        let mut parent_hash = None;
        for _i in 0..level {
            index_key = temp_index.parent();
            if let Some(internal_parent_hash) = self.get_node_index(index_key) {
                parent_hash = Some(internal_parent_hash);
                break;
            }
            temp_index = temp_index.parent();
        }
        // get node by hash
        let parent_hash = parent_hash.unwrap_or(self.root_hash);
        let mut hash_vec = vec![parent_hash];

        while let Some(temp_node_hash) = hash_vec.pop() {
            match self.get_node(temp_node_hash)? {
                Some(AccumulatorNode::Internal(internal)) => {
                    let internal_index = internal.index();
                    if internal_index == index {
                        return Ok(internal.hash());
                    } else if internal_index == index.parent() {
                        if internal_index.left_child() == index {
                            return Ok(internal.left());
                        }
                        if internal_index.right_child() == index {
                            return Ok(internal.right());
                        }
                    } else if internal_index.to_inorder_index() > index.to_inorder_index() {
                        //current internal node is left part
                        if internal.left() != *ACCUMULATOR_PLACEHOLDER_HASH
                            && !internal_index.left_child().is_leaf()
                        {
                            hash_vec.push(internal.left());
                        }
                    } else {
                        //current internal node is left part
                        if internal.right() != *ACCUMULATOR_PLACEHOLDER_HASH
                            && !internal_index.right_child().is_leaf()
                        {
                            hash_vec.push(internal.right());
                        }
                    }
                }
                Some(AccumulatorNode::Leaf(leaf)) => {
                    if leaf.index() == index {
                        return Ok(leaf.value());
                    }
                }
                _ => {
                    bail!(
                        "can not find accumulator node by hash :{:?} in store: {:?}",
                        temp_node_hash,
                        self.store.store_type()
                    );
                }
            }
        }
        bail!("node hash not found:{:?}", index)
    }

    fn save_node_indexes(&mut self, nodes: Vec<AccumulatorNode>) {
        let id = format!("{:p}", self);
        let cache = &mut self.index_cache;
        for node in nodes {
            if let Some(old) = cache.put(node.index(), node.hash()) {
                tracing::trace!("cache exist node hash: {}-{:?}-{:?}", id, node.index(), old);
            }
        }
    }

    fn rightmost_leaf_index(&self) -> u64 {
        if self.num_leaves == 0 {
            0_u64
        } else {
            self.num_leaves - 1
        }
    }

    /// upper bound of num of frozen nodes:
    ///     new leaves and resulting frozen internal nodes forming a complete binary subtree
    ///         num_new_leaves * 2 - 1 < num_new_leaves * 2
    ///     and the full route from root of that subtree to the accumulator root turns frozen
    ///         height - (log2(num_new_leaves) + 1) < height - 1 = root_level
    fn max_to_freeze(num_new_leaves: usize, root_level: u32) -> usize {
        precondition!(root_level as usize <= MAX_ACCUMULATOR_PROOF_DEPTH);
        precondition!(num_new_leaves < (usize::MAX / 2));
        precondition!(num_new_leaves * 2 <= usize::MAX - root_level as usize);
        num_new_leaves * 2 + root_level as usize
    }

    #[cfg(test)]
    pub fn get_index_frozen_subtrees(&self) -> HashMap<NodeIndex, H256> {
        self.index_frozen_subtrees.clone()
    }
}
