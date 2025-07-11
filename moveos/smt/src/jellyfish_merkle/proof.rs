// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::hash::*;
use super::node_type::{SparseMerkleInternalNode, SparseMerkleLeafNode};
use crate::{Key, Value};
use anyhow::{Result, bail, ensure};
use primitive_types::H256;
use serde::{Deserialize, Serialize};

/// A proof that can be used to authenticate an element in a Sparse Merkle Tree given trusted root
/// hash. For example, `TransactionInfoToAccountProof` can be constructed on top of this structure.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseMerkleProof {
    /// This proof can be used to authenticate whether a given leaf exists in the tree or not.
    ///     - If this is `Some(H256, H256)`
    ///         - If the first `H256` equals requested key, this is an inclusion proof and the
    ///           second `H256` equals the hash of the corresponding account blob.
    ///         - Otherwise this is a non-inclusion proof. The first `H256` is the only key
    ///           that exists in the subtree and the second `H256` equals the hash of the
    ///           corresponding blob.
    ///     - If this is `None`, this is also a non-inclusion proof which indicates the subtree is
    ///       empty.
    pub leaf: Option<(H256, H256)>,

    /// All siblings in this proof, including the default ones. Siblings are ordered from the bottom
    /// level to the root level.
    pub siblings: Vec<H256>,
}

impl SparseMerkleProof {
    /// Constructs a new `SparseMerkleProof` using leaf and a list of siblings.
    pub fn new(leaf: Option<(H256, H256)>, siblings: Vec<H256>) -> Self {
        SparseMerkleProof { leaf, siblings }
    }

    /// Returns the leaf node in this proof.
    pub fn leaf(&self) -> Option<(H256, H256)> {
        self.leaf
    }

    /// Returns the list of siblings in this proof.
    pub fn siblings(&self) -> &[H256] {
        &self.siblings
    }

    /// If `element_blob` is present, verifies an element whose key is `element_key` and value is
    /// `element_blob` exists in the Sparse Merkle Tree using the provided proof. Otherwise
    /// verifies the proof is a valid non-inclusion proof that shows this key doesn't exist in the
    /// tree.
    pub fn verify<K: Key, V: Value>(
        &self,
        expected_root_hash: H256,
        element_key: K,
        element_blob: Option<V>,
    ) -> Result<()> {
        ensure!(
            self.siblings.len() <= SMTNodeHash::LEN_IN_BITS,
            "Sparse Merkle Tree proof has more than {} ({}) siblings.",
            SMTNodeHash::LEN_IN_BITS,
            self.siblings.len(),
        );
        let element_key_hash = element_key.merkle_hash();

        match (element_blob, self.leaf) {
            (Some(blob), Some((proof_key, proof_value_hash))) => {
                // This is an inclusion proof, so the key and value hash provided in the proof
                // should match element_key and element_value_hash. `siblings` should prove the
                // route from the leaf node to the root.
                ensure!(
                    element_key_hash == proof_key,
                    "Keys do not match. Key in proof: {:x}. Expected key: {:x}.",
                    proof_key,
                    element_key_hash
                );
                let hash: H256 = blob.into_object()?.merkle_hash().into();
                ensure!(
                    hash == proof_value_hash,
                    "Value hashes do not match. Value hash in proof: {:x}. \
                     Expected value hash: {:x}",
                    proof_value_hash,
                    hash,
                );
            }
            (Some(_blob), None) => bail!("Expected inclusion proof. Found non-inclusion proof."),
            (None, Some((proof_key, _))) => {
                // This is a non-inclusion proof. The proof intends to show that if a leaf node
                // representing `element_key` is inserted, it will break a currently existing leaf
                // node represented by `proof_key` into a branch. `siblings` should prove the
                // route from that leaf node to the root.
                ensure!(
                    element_key_hash != proof_key,
                    "Expected non-inclusion proof, but key exists in proof.",
                );
                ensure!(
                    element_key_hash.common_prefix_bits_len(proof_key.into())
                        >= self.siblings.len(),
                    "Key would not have ended up in the subtree where the provided key in proof \
                     is the only existing key, if it existed. So this is not a valid \
                     non-inclusion proof.",
                );
            }
            (None, None) => {
                // This is a non-inclusion proof. The proof intends to show that if a leaf node
                // representing `element_key` is inserted, it will show up at a currently empty
                // position. `sibling` should prove the route from this empty position to the root.
            }
        }

        let current_hash = self.leaf.map_or(
            *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
            |(key, value_hash)| {
                SparseMerkleLeafNode::new(key.into(), value_hash.into()).merkle_hash()
            },
        );

        let actual_root_hash = self
            .siblings
            .iter()
            .zip(
                element_key_hash
                    .iter_bits()
                    .rev()
                    .skip(SMTNodeHash::LEN_IN_BITS - self.siblings.len()),
            )
            .fold(current_hash, |hash, (sibling_hash, bit)| {
                if bit {
                    SparseMerkleInternalNode::new((*sibling_hash).into(), hash).merkle_hash()
                } else {
                    SparseMerkleInternalNode::new(hash, (*sibling_hash).into()).merkle_hash()
                }
            });
        ensure!(
            actual_root_hash == expected_root_hash,
            "Root hashes do not match. Actual root hash: {:x}. Expected root hash: {:x}.",
            actual_root_hash,
            expected_root_hash,
        );

        Ok(())
    }

    /// Update the leaf, and compute new root.
    /// Only available for non existence proof
    pub fn update_leaf<K: Key, V: Value>(
        &mut self,
        element_key: K,
        element_blob: V,
    ) -> Result<H256> {
        let element_key_hash = element_key.merkle_hash();
        let element_hash = element_blob.into_object()?.merkle_hash();
        let is_non_exists_proof = match self.leaf.as_ref() {
            None => true,
            Some((leaf_key, _leaf_value)) => &element_key_hash != leaf_key,
        };
        ensure!(
            is_non_exists_proof,
            "Only non existence proof support update leaf, got element_key hash: {:?} leaf: {:?}",
            element_key_hash,
            self.leaf,
        );

        let new_leaf_node = SparseMerkleLeafNode::new(element_key_hash, element_hash);
        let current_hash = new_leaf_node.merkle_hash();
        if let Some(leaf_node) = self.leaf.as_ref().map(|(leaf_key, leaf_value)| {
            SparseMerkleLeafNode::new((*leaf_key).into(), (*leaf_value).into())
        }) {
            let mut new_siblings: Vec<H256> = vec![leaf_node.merkle_hash().into()];
            let prefix_len = leaf_node.key_hash.common_prefix_bits_len(element_key_hash);

            let place_holder_len = (prefix_len - self.siblings.len()) + 1;
            if place_holder_len > 0 {
                new_siblings.resize(place_holder_len, *SPARSE_MERKLE_PLACEHOLDER_HASH);
            }
            new_siblings.extend(self.siblings.iter());
            self.siblings = new_siblings;
        }
        let new_root_hash = self
            .siblings
            .iter()
            .zip(
                element_key_hash
                    .iter_bits()
                    .rev()
                    .skip(SMTNodeHash::LEN_IN_BITS - self.siblings.len()),
            )
            .fold(current_hash, |hash, (sibling_hash, bit)| {
                if bit {
                    SparseMerkleInternalNode::new((*sibling_hash).into(), hash).merkle_hash()
                } else {
                    SparseMerkleInternalNode::new(hash, (*sibling_hash).into()).merkle_hash()
                }
            });
        self.leaf = Some((element_key_hash.into(), element_hash.into()));
        Ok(new_root_hash.into())
    }
}

/// A proof that can be used authenticate a range of consecutive leaves, from the leftmost leaf to
/// a certain one, in a sparse Merkle tree. For example, given the following sparse Merkle tree:
///
/// ```text
///                   root
///                  /     \
///                 /       \
///                /         \
///               o           o
///              / \         / \
///             a   o       o   h
///                / \     / \
///               o   d   e   X
///              / \         / \
///             b   c       f   g
/// ```
///
/// if the proof wants show that `[a, b, c, d, e]` exists in the tree, it would need the siblings
/// `X` and `h` on the right.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseMerkleRangeProof {
    /// The vector of siblings on the right of the path from root to last leaf. The ones near the
    /// bottom are at the beginning of the vector. In the above example, it's `[X, h]`.
    right_siblings: Vec<H256>,
}

impl SparseMerkleRangeProof {
    /// Constructs a new `SparseMerkleRangeProof`.
    pub fn new(right_siblings: Vec<H256>) -> Self {
        Self { right_siblings }
    }

    /// Returns the siblings.
    pub fn right_siblings(&self) -> &[H256] {
        &self.right_siblings
    }
}
