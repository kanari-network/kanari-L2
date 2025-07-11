// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
#![allow(dead_code)]
//TODO fix
#![allow(clippy::unit_arg)]
//! This module implements [`JellyfishMerkleTree`] backed by storage module. The tree itself doesn't
//! persist anything, but realizes the logic of R/W only. The write path will produce all the
//! intermediate results in a batch for storage layer to commit and the read path will return
//! results directly. The public APIs are only [`new`], [`put_blob_sets`], [`put_blob_set`] and
//! [`get_with_proof`]. After each put with a `blob_set` based on a known version, the tree will
//! return a new root hash with a [`TreeUpdateBatch`] containing all the new nodes and indices of
//! stale nodes.
//!
//! A Jellyfish Merkle Tree itself logically is a 256-bit sparse Merkle tree with an optimization
//! that any subtree containing 0 or 1 leaf node will be replaced by that leaf node or a placeholder
//! node with default hash value. With this optimization we can save CPU by avoiding hashing on
//! many sparse levels in the tree. Physically, the tree is structurally similar to the modified
//! Patricia Merkle tree of Ethereum but with some modifications. A standard Jellyfish Merkle tree
//! will look like the following figure:
//!
//! ```text
//!                                    .──────────────────────.
//!                            _.─────'                        `──────.
//!                       _.──'                                        `───.
//!                   _.─'                                                  `──.
//!               _.─'                                                          `──.
//!             ,'                                                                  `.
//!          ,─'                                                                      '─.
//!        ,'                                                                            `.
//!      ,'                                                                                `.
//!     ╱                                                                                    ╲
//!    ╱                                                                                      ╲
//!   ╱                                                                                        ╲
//!  ╱                                                                                          ╲
//! ;                                                                                            :
//! ;                                                                                            :
//! ;                                                                                              :
//! │                                                                                              │
//! +──────────────────────────────────────────────────────────────────────────────────────────────+
//! .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.  .''.
//! /    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \/    \
//! +----++----++----++----++----++----++----++----++----++----++----++----++----++----++----++----+
//! (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (
//!  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )
//! (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (
//!  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )
//! (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (
//!  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )
//! (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (
//!  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )  )
//! (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (  (
//! ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■  ■
//! ■: account_state_blob
//! ```
//!
//! A Jellyfish Merkle Tree consists of [`InternalNode`] and [`LeafNode`]. [`InternalNode`] is like
//! branch node in ethereum patricia merkle with 16 children to represent a 4-level binary tree and
//! [`LeafNode`] is similar to that in patricia merkle too. In the above figure, each `bell` in the
//! jellyfish is an [`InternalNode`] while each tentacle is a [`LeafNode`]. It is noted that
//! Jellyfish merkle doesn't have a counterpart for `extension` node of ethereum patricia merkle.
//!
//! [`JellyfishMerkleTree`]: struct.JellyfishMerkleTree.html
//! [`new`]: struct.JellyfishMerkleTree.html#method.new
//! [`put_blob_sets`]: struct.JellyfishMerkleTree.html#method.put_blob_sets
//! [`put_blob_set`]: struct.JellyfishMerkleTree.html#method.put_blob_set
//! [`get_with_proof`]: struct.JellyfishMerkleTree.html#method.get_with_proof
//! [`TreeUpdateBatch`]: struct.TreeUpdateBatch.html
//! [`InternalNode`]: node_type/struct.InternalNode.html
//! [`LeafNode`]: node_type/struct.LeafNode.html

pub(crate) mod hash;
pub(crate) mod iterator;
#[cfg(test)]
pub(crate) mod jellyfish_merkle_test;
pub(crate) mod mock_tree_store;
pub(crate) mod nibble;
pub(crate) mod nibble_path;
pub(crate) mod node_type;
pub mod proof;
pub(crate) mod test_helper;
pub(crate) mod tree_cache;

use crate::{Key, SMTObject, Value};
use anyhow::{Result, bail, ensure, format_err};
use backtrace::Backtrace;
use hash::{Hash, SMTHash, SMTNodeHash};
use nibble_path::{NibbleIterator, NibblePath, skip_common_prefix};
use node_type::{Child, Children, InternalNode, LeafNode, Node, NodeKey};
use primitive_types::H256;
use proof::{SparseMerkleProof, SparseMerkleRangeProof};
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use tracing::debug;
use tree_cache::TreeCache;

/// The hardcoded maximum height of a [`JellyfishMerkleTree`] in nibbles.
pub const ROOT_NIBBLE_HEIGHT: usize = SMTNodeHash::LEN * 2;

/// `TreeReader` defines the interface between
/// [`JellyfishMerkleTree`](struct.JellyfishMerkleTree.html)
/// and underlying storage holding nodes.
pub(crate) trait TreeReader<K, V> {
    /// Gets node given a node key. Returns error if the node does not exist.
    fn get_node(&self, node_key: &NodeKey) -> Result<Node<K, V>> {
        self.get_node_option(node_key)?.ok_or_else(|| {
            let backtrace = format!("{:#?}", Backtrace::new());
            debug!("backtrace: {}", backtrace);
            format_err!("Missing node at {:?}.", node_key)
        })
    }

    /// Gets node given a node key. Returns `None` if the node does not exist.
    fn get_node_option(&self, node_key: &NodeKey) -> Result<Option<Node<K, V>>>;
}

pub(crate) trait TreeWriter<K, V> {
    /// Writes a node batch into storage.
    fn write_node_batch(&self, node_batch: &NodeBatch<K, V>) -> Result<()>;
}

/// Node batch that will be written into db atomically with other batches.
pub(crate) type NodeBatch<K, V> = BTreeMap<NodeKey, Node<K, V>>;
/// [`StaleNodeIndex`](struct.StaleNodeIndex.html) batch that will be written into db atomically
/// with other batches.
pub(crate) type StaleNodeIndexBatch = BTreeSet<StaleNodeIndex>;

/// Indicates a node becomes stale since `stale_since_version`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StaleNodeIndex {
    /// The version since when the node is overwritten and becomes stale.
    pub stale_since_version: SMTNodeHash,
    /// The [`NodeKey`](node_type/struct.NodeKey.html) identifying the node associated with this
    /// record.
    pub node_key: NodeKey,
}

/// This is a wrapper of [`NodeBatch`](type.NodeBatch.html),
/// [`StaleNodeIndexBatch`](type.StaleNodeIndexBatch.html) and some stats of nodes that represents
/// the incremental updates of a tree and pruning indices after applying a write set,
/// which is a vector of `K` and `V` pairs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TreeUpdateBatch<K, V> {
    pub node_batch: NodeBatch<K, V>,
    pub stale_node_index_batch: StaleNodeIndexBatch,
    pub num_new_leaves: usize,
    pub num_stale_leaves: usize,
}

impl<K, V> Default for TreeUpdateBatch<K, V> {
    fn default() -> Self {
        Self {
            node_batch: NodeBatch::default(),
            stale_node_index_batch: StaleNodeIndexBatch::default(),
            num_new_leaves: 0,
            num_stale_leaves: 0,
        }
    }
}

/// The Jellyfish Merkle tree data structure. See [`crate`] for description.
pub(crate) struct JellyfishMerkleTree<'a, K, V, R: 'a + TreeReader<K, V>> {
    reader: &'a R,
    key: PhantomData<K>,
    value: PhantomData<V>,
}

impl<'a, K, V, R> JellyfishMerkleTree<'a, K, V, R>
where
    K: Key,
    V: Value,
    R: 'a + TreeReader<K, V>,
{
    /// Creates a `JellyfishMerkleTree` backed by the given [`TreeReader`](trait.TreeReader.html).
    pub fn new(reader: &'a R) -> Self {
        Self {
            reader,
            key: PhantomData,
            value: PhantomData,
        }
    }

    #[cfg(test)]
    pub fn put_blob_set(
        &self,
        state_root_hash: Option<SMTNodeHash>,
        blob_set: Vec<(K, SMTObject<V>)>,
    ) -> Result<(SMTNodeHash, TreeUpdateBatch<K, V>)> {
        let blob_set = blob_set
            .into_iter()
            .map(|(k, v)| (k, Some(v)))
            .collect::<Vec<_>>();
        self.updates(state_root_hash, blob_set)
    }

    #[cfg(test)]
    pub fn print_tree<PK: Into<K>>(
        &self,
        state_root_hash: SMTNodeHash,
        start_key: PK,
    ) -> Result<()> {
        let iter = self::iterator::JellyfishMerkleIterator::new(
            self.reader,
            state_root_hash,
            Some(start_key.into()),
        )?;
        iter.print()
    }

    /// Delete a key from the tree. If the key is not found in the tree, nothing happens.
    #[cfg(test)]
    pub fn delete<DK: Into<K>>(
        &self,
        state_root_hash: Option<SMTNodeHash>,
        key: DK,
    ) -> Result<(SMTNodeHash, TreeUpdateBatch<K, V>)> {
        self.updates(state_root_hash, vec![(key.into(), None)])
    }

    /// Insert all kvs in `blob_set` into tree, return updated root hash and tree updates
    pub fn insert_all(
        &self,
        state_root_hash: Option<SMTNodeHash>,
        blob_set: Vec<(K, SMTObject<V>)>,
    ) -> Result<(SMTNodeHash, TreeUpdateBatch<K, V>)> {
        let blob_set = blob_set
            .into_iter()
            .map(|(k, v)| (k, Some(v)))
            .collect::<Vec<_>>();
        self.updates(state_root_hash, blob_set)
    }

    pub fn updates<S: Into<Vec<(K, Option<SMTObject<V>>)>>>(
        &self,
        state_root_hash: Option<SMTNodeHash>,
        blob_set: S,
    ) -> Result<(SMTNodeHash, TreeUpdateBatch<K, V>)> {
        let (root_hashes, tree_update_batch) = self.puts(state_root_hash, vec![blob_set.into()])?;
        assert_eq!(
            root_hashes.len(),
            1,
            "root_hashes must consist of a single value.",
        );
        Ok((root_hashes[0], tree_update_batch))
    }

    /// Returns the new nodes and account state blobs in a batch after applying `blob_set`. For
    /// example, if after transaction `T_i` the committed state of tree in the persistent storage
    /// looks like the following structure:
    ///
    /// ```text
    ///              S_i
    ///             /   \
    ///            .     .
    ///           .       .
    ///          /         \
    ///         o           x
    ///        / \
    ///       A   B
    ///        storage (disk)
    /// ```
    ///
    /// where `A` and `B` denote the states of two adjacent accounts, and `x` is a sibling subtree
    /// of the path from root to A and B in the tree. Then a `blob_set` produced by the next
    /// transaction `T_{i+1}` modifies other accounts `C` and `D` exist in the subtree under `x`, a
    /// new partial tree will be constructed in memory and the structure will be:
    ///
    /// ```text
    ///                 S_i      |      S_{i+1}
    ///                /   \     |     /       \
    ///               .     .    |    .         .
    ///              .       .   |   .           .
    ///             /         \  |  /             \
    ///            /           x | /               x'
    ///           o<-------------+-               / \
    ///          / \             |               C   D
    ///         A   B            |
    ///           storage (disk) |    cache (memory)
    /// ```
    ///
    /// With this design, we are able to query the global state in persistent storage and
    /// generate the proposed tree delta based on a specific root hash and `blob_set`. For
    /// example, if we want to execute another transaction `T_{i+1}'`, we can use the tree `S_i` in
    /// storage and apply the `blob_set` of transaction `T_{i+1}`. Then if the storage commits
    /// the returned batch, the state `S_{i+1}` is ready to be read from the tree by calling
    /// [`get_with_proof`](struct.JellyfishMerkleTree.html#method.get_with_proof). Anything inside
    /// the batch is not reachable from public interfaces before being committed.
    //FIXME fix clippy warning
    #[allow(clippy::type_complexity)]
    fn puts(
        &self,
        state_root_hash: Option<SMTNodeHash>,
        blob_sets: Vec<Vec<(K, Option<SMTObject<V>>)>>,
    ) -> Result<(Vec<SMTNodeHash>, TreeUpdateBatch<K, V>)> {
        let mut tree_cache = TreeCache::new(self.reader, state_root_hash);
        for blob_set in blob_sets.into_iter() {
            assert!(
                !blob_set.is_empty(),
                "Transactions that output empty write set should not be included.",
            );
            blob_set
                .into_iter()
                .try_for_each(|(key, blob)| Self::put(key, blob, &mut tree_cache))?;
            // Freezes the current cache to make all contents in the current cache immutable.
            // TODO: maybe we should not freeze, check here again.
            tree_cache.freeze();
        }

        Ok(tree_cache.into())
    }

    fn put(key: K, blob: Option<SMTObject<V>>, tree_cache: &mut TreeCache<R, K, V>) -> Result<()> {
        let key_hash = key.merkle_hash();
        let nibble_path = NibblePath::new(key_hash.to_vec());

        // Get the root node. If this is the first operation, it would get the root node from the
        // underlying db. Otherwise it most likely would come from `cache`.
        let root_node_key = tree_cache.get_root_node_key();
        let mut nibble_iter = nibble_path.nibbles();

        // Start insertion from the root node.
        let (new_root_node_key, _) =
            Self::insert_at(*root_node_key, &mut nibble_iter, key, blob, tree_cache)?;

        tree_cache.set_root_node_key(new_root_node_key);
        Ok(())
    }

    /// Helper function for recursive insertion into the subtree that starts from the current
    /// [`NodeKey`](node_type/struct.NodeKey.html). Returns the newly inserted node.
    /// It is safe to use recursion here because the max depth is limited by the key length which
    /// for this tree is the length of the hash of account addresses.
    fn insert_at(
        node_key: NodeKey,
        nibble_iter: &mut NibbleIterator,
        key: K,
        blob: Option<SMTObject<V>>,
        tree_cache: &mut TreeCache<R, K, V>,
    ) -> Result<(NodeKey, Node<K, V>)> {
        let node = tree_cache.get_node(&node_key)?;
        match node {
            Node::Internal(internal_node) => Self::insert_at_internal_node(
                node_key,
                internal_node,
                nibble_iter,
                key,
                blob,
                tree_cache,
            ),
            Node::Leaf(leaf_node) => {
                Self::insert_at_leaf_node(node_key, leaf_node, nibble_iter, key, blob, tree_cache)
            }
            Node::Null => match blob {
                None => Ok((node_key, node)),
                Some(blob) => {
                    tree_cache.delete_node(&node_key, false);
                    Self::create_leaf_node(key, blob, tree_cache)
                }
            },
        }
    }

    /// Helper function for recursive insertion into the subtree that starts from the current
    /// `internal_node`. Returns the newly inserted node with its
    /// [`NodeKey`](node_type/struct.NodeKey.html).
    fn insert_at_internal_node(
        node_key: NodeKey,
        internal_node: InternalNode,
        nibble_iter: &mut NibbleIterator,
        key: K,
        blob: Option<SMTObject<V>>,
        tree_cache: &mut TreeCache<R, K, V>,
    ) -> Result<(NodeKey, Node<K, V>)> {
        // Find the next node to visit following the next nibble as index.
        let child_index = nibble_iter.next().expect("Ran out of nibbles");

        // Traverse downwards from this internal node recursively to get the `node_key` of the child
        // node at `child_index`.
        let (new_child_key, new_child_node) = match internal_node.child(child_index) {
            Some(child) => {
                // let child_node_key = node_key.gen_child_node_key(child.version, child_index);
                let child_node_key = child.hash;
                Self::insert_at(child_node_key, nibble_iter, key, blob, tree_cache)?
            }
            None if blob.is_some() => {
                let blob = blob.expect("blob must be some at here");
                // let new_child_node_key = node_key.gen_child_node_key(version, child_index);
                Self::create_leaf_node(key, blob, tree_cache)?
            }
            _ => return Ok((node_key, Node::from(internal_node))),
        };

        // we can use node_key without recompute node hash
        let child_not_changed = internal_node
            .child(child_index)
            .filter(|old| old.hash == new_child_key && old.is_leaf == new_child_node.is_leaf())
            .is_some();

        // don't need to prune it if no change happens.
        if child_not_changed {
            return Ok((node_key, Node::from(internal_node)));
        }

        // now, we need to reconstruct internal node.
        tree_cache.delete_node(&node_key, false);

        // Reuse the current `InternalNode` in memory to create a new internal node.
        let mut children: Children = internal_node.into();
        children.remove(&child_index);

        match &new_child_node {
            Node::Null => {}
            _ => {
                children.insert(
                    child_index,
                    Child::new(new_child_key, new_child_node.is_leaf()),
                );
            }
        }

        if children.is_empty() {
            let empty_node = Node::new_null();
            Ok((empty_node.merkle_hash(), empty_node))
        } else if children.len() == 1
            && children
                .values()
                .next()
                .expect("must exist one child")
                .is_leaf
        {
            let (_, leaf) = children.into_iter().next().expect("must exist one child");
            let leaf_node = tree_cache.get_node(&leaf.hash)?;
            Ok((leaf.hash, leaf_node))
        } else {
            let new_internal_node: Node<K, V> = InternalNode::new(children).into();
            // Cache this new internal node.
            tree_cache.put_node(new_internal_node.merkle_hash(), new_internal_node.clone())?;
            Ok((new_internal_node.merkle_hash(), new_internal_node))
        }
    }

    /// Helper function for recursive insertion into the subtree that starts from the
    /// `existing_leaf_node`. Returns the newly inserted node with its
    /// [`NodeKey`](node_type/struct.NodeKey.html).
    fn insert_at_leaf_node(
        node_key: NodeKey,
        existing_leaf_node: LeafNode<K, V>,
        nibble_iter: &mut NibbleIterator,
        key: K,
        blob: Option<SMTObject<V>>,
        tree_cache: &mut TreeCache<R, K, V>,
    ) -> Result<(NodeKey, Node<K, V>)> {
        // We are on a leaf node but trying to insert another node, so we may diverge.
        // We always delete the existing leaf node here because it will not be referenced anyway
        // since this version.

        // 1. Make sure that the existing leaf nibble_path has the same prefix as the already
        // visited part of the nibble iter of the incoming key and advances the existing leaf
        // nibble iterator by the length of that prefix.
        let mut visited_nibble_iter = nibble_iter.visited_nibbles();
        let existing_leaf_nibble_path = NibblePath::new(existing_leaf_node.key_hash().to_vec());
        let mut existing_leaf_nibble_iter = existing_leaf_nibble_path.nibbles();
        skip_common_prefix(&mut visited_nibble_iter, &mut existing_leaf_nibble_iter);

        // TODO(lightmark): Change this to corrupted error.
        assert!(
            visited_nibble_iter.is_finished(),
            "Leaf nodes failed to share the same visited nibbles before index {}",
            existing_leaf_nibble_iter.visited_nibbles().num_nibbles()
        );

        // 2. Determine the extra part of the common prefix that extends from the position where
        // step 1 ends between this leaf node and the incoming key.
        let mut existing_leaf_nibble_iter_below_internal =
            existing_leaf_nibble_iter.remaining_nibbles();
        let num_common_nibbles_below_internal =
            skip_common_prefix(nibble_iter, &mut existing_leaf_nibble_iter_below_internal);
        let mut common_nibble_path = nibble_iter.visited_nibbles().collect::<NibblePath>();

        // 2.1. Both are finished. That means the incoming key already exists in the tree and we
        // just need to update its value.
        if nibble_iter.is_finished() {
            assert!(existing_leaf_nibble_iter_below_internal.is_finished());
            if blob.is_none() {
                tree_cache.delete_node(&node_key, true);
                let empty_node = Node::new_null();
                return Ok((empty_node.merkle_hash(), empty_node));
            }
            let blob = blob.expect("blob must some at here");
            // The new leaf node will have the same nibble_path with a new version as node_key.
            // if the blob are same, return directly
            if blob.merkle_hash() == existing_leaf_node.value_hash() {
                return Ok((node_key, Node::Leaf(existing_leaf_node)));
            } else {
                // Else create the new leaf node with the same address but new blob content.
                tree_cache.delete_node(&node_key, true /* is_leaf */);
                return Self::create_leaf_node(key, blob, tree_cache);
            }
        }

        // 2.2. both are unfinished(They have keys with same length so it's impossible to have one
        // finished and the other not). This means the incoming key forks at some point between the
        // position where step 1 ends and the last nibble, inclusive. Then create a series of
        // internal nodes the number of which equals to the length of the extra part of the
        // common prefix in step 2, a new leaf node for the incoming key, and update the
        // [`NodeKey`] of existing leaf node. We create new internal nodes in a bottom-up
        // order.
        let existing_leaf_index = existing_leaf_nibble_iter_below_internal
            .next()
            .expect("Ran out of nibbles");
        let new_leaf_index = nibble_iter.next().expect("Ran out of nibbles");
        assert_ne!(existing_leaf_index, new_leaf_index);

        // if this is a delete, the key is not found in tree, so we just return the origin node
        if blob.is_none() {
            return Ok((node_key, Node::from(existing_leaf_node)));
        }
        let blob = blob.expect("blob must some at here");

        let mut children = Children::new();
        children.insert(
            existing_leaf_index,
            Child::new(existing_leaf_node.merkle_hash(), true /* is_leaf */),
        );

        let (_, new_leaf_node) = Self::create_leaf_node(key, blob, tree_cache)?;
        children.insert(
            new_leaf_index,
            Child::new(new_leaf_node.merkle_hash(), true /* is_leaf */),
        );

        let internal_node = InternalNode::new(children);
        let mut next_internal_node = internal_node.clone();
        let internal_node: Node<K, V> = internal_node.into();
        tree_cache.put_node(internal_node.merkle_hash(), internal_node)?;

        for _i in 0..num_common_nibbles_below_internal {
            let nibble = common_nibble_path
                .pop()
                .expect("Common nibble_path below internal node ran out of nibble");
            let mut children = Children::new();
            children.insert(
                nibble,
                Child::new(next_internal_node.merkle_hash(), false /* is_leaf */),
            );
            let internal_node = InternalNode::new(children);
            next_internal_node = internal_node.clone();
            let internal_node: Node<K, V> = internal_node.into();
            tree_cache.put_node(internal_node.merkle_hash(), internal_node)?;
        }

        let next_internal_node: Node<K, V> = next_internal_node.into();
        Ok((next_internal_node.merkle_hash(), next_internal_node))
    }

    /// Helper function for creating leaf nodes. Returns the newly created leaf node.
    fn create_leaf_node(
        key: K,
        blob: SMTObject<V>,
        tree_cache: &mut TreeCache<R, K, V>,
    ) -> Result<(NodeKey, Node<K, V>)> {
        // Get the underlying bytes of nibble_iter which must be a key, i.e., hashed account address
        // with `SMTNodeHash::LEN` bytes.
        let new_leaf_node = Node::new_leaf(key, blob);
        let node_key = new_leaf_node.merkle_hash();
        tree_cache.put_node(node_key, new_leaf_node.clone())?;
        Ok((node_key, new_leaf_node))
    }

    /// Returns the account state blob (if applicable) and the corresponding merkle proof.
    pub fn get_with_proof<GK: Into<K>>(
        &self,
        state_root_hash: SMTNodeHash,
        key: GK,
    ) -> Result<(Option<SMTObject<V>>, SparseMerkleProof)> {
        // Empty tree just returns proof with no sibling hash.
        // let mut next_node_key = NodeKey::new_empty_path(version);
        let mut next_node_key = state_root_hash;
        let mut siblings: Vec<H256> = vec![];

        // We use key's hash as nibble_path, not origin key bytes, make smt more distributed
        let key = key.into();
        let path_bytes = key.merkle_hash().to_vec();
        let nibble_path = NibblePath::new(path_bytes);
        let mut nibble_iter = nibble_path.nibbles();

        // We limit the number of loops here deliberately to avoid potential cyclic graph bugs
        // in the tree structure.
        for nibble_depth in 0..=ROOT_NIBBLE_HEIGHT {
            let next_node = self.reader.get_node(&next_node_key)?;
            match next_node {
                Node::Internal(internal_node) => {
                    let queried_child_index = nibble_iter
                        .next()
                        .ok_or_else(|| format_err!("ran out of nibbles"))?;
                    let (child_node_key, siblings_in_internal) =
                        internal_node.get_child_with_siblings(queried_child_index);
                    //TODO optimize
                    let mut siblings_in_internal: Vec<H256> =
                        siblings_in_internal.into_iter().map(|s| s.into()).collect();
                    siblings.append(&mut siblings_in_internal);
                    if let Some(node_key) = child_node_key {
                        next_node_key = node_key;
                    } else {
                        return Ok((
                            None,
                            SparseMerkleProof::new(None, {
                                siblings.reverse();
                                siblings
                            }),
                        ));
                    }
                }
                Node::Leaf(leaf_node) => {
                    return Ok((
                        if leaf_node.key_hash() == key.merkle_hash() {
                            Some(leaf_node.value().clone())
                        } else {
                            None
                        },
                        SparseMerkleProof::new(
                            Some((leaf_node.key_hash().into(), leaf_node.value_hash().into())),
                            {
                                siblings.reverse();
                                siblings
                            },
                        ),
                    ));
                }
                Node::Null => {
                    if nibble_depth == 0 {
                        return Ok((None, SparseMerkleProof::new(None, vec![])));
                    } else {
                        bail!(
                            "Non-root null node exists with node key {:?}",
                            next_node_key
                        );
                    }
                }
            }
        }
        bail!("Jellyfish Merkle tree has cyclic graph inside.");
    }

    /// Gets the proof that shows a list of keys up to `rightmost_key_to_prove` exist at `version`.
    pub fn get_range_proof(
        &self,
        state_root_hash: SMTNodeHash,
        rightmost_key_to_prove: K,
    ) -> Result<SparseMerkleRangeProof> {
        let key_hash = rightmost_key_to_prove.merkle_hash();
        let (account, proof) = self.get_with_proof(state_root_hash, rightmost_key_to_prove)?;
        ensure!(account.is_some(), "rightmost_key_to_prove must exist.");

        let siblings = proof
            .siblings()
            .iter()
            .rev()
            .zip(key_hash.iter_bits())
            .filter_map(|(sibling, bit)| {
                // We only need to keep the siblings on the right.
                if !bit { Some(*sibling) } else { None }
            })
            .rev()
            .collect();
        Ok(SparseMerkleRangeProof::new(siblings))
    }

    #[cfg(test)]
    pub fn get<GK: Into<K>>(
        &self,
        state_root_hash: SMTNodeHash,
        key: GK,
    ) -> Result<Option<SMTObject<V>>> {
        Ok(self.get_with_proof(state_root_hash, key)?.0)
    }
}
