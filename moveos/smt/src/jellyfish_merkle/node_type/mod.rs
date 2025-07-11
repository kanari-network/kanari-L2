// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Node types of [`JellyfishMerkleTree`](super::JellyfishMerkleTree)
//!
//! This module defines two types of Jellyfish Merkle tree nodes: [`InternalNode`]
//! and [`LeafNode`] as building blocks of a 256-bit
//! [`JellyfishMerkleTree`](super::JellyfishMerkleTree). [`InternalNode`] represents a 4-level
//! binary tree to optimize for IOPS: it compresses a tree with 31 nodes into one node with 16
//! chidren at the lowest level. [`LeafNode`] stores the full key and the account blob data
//! associated.
#![allow(clippy::unit_arg)]

#[cfg(test)]
mod node_type_test;
use super::hash::*;
use super::nibble::Nibble;
use crate::{Key, SMTObject, Value};
use anyhow::{Context, Result, ensure};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::cast::FromPrimitive;
use primitive_types::H256;
#[cfg(any(test, feature = "fuzzing"))]
use proptest::{collection::hash_map, prelude::*};
#[cfg(any(test, feature = "fuzzing"))]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::{
    collections::hash_map::HashMap,
    io::{Cursor, Read, SeekFrom, prelude::*},
    mem::size_of,
};
use thiserror::Error;

pub(crate) type NodeKey = SMTNodeHash;

/// Each child of [`InternalNode`] encapsulates a nibble forking at this node.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "fuzzing"), derive(Arbitrary))]
pub(crate) struct Child {
    // The hash value of this child node.
    pub hash: SMTNodeHash,
    // Whether the child is a leaf node.
    pub is_leaf: bool,
}

impl Child {
    pub fn new(hash: SMTNodeHash, is_leaf: bool) -> Self {
        Self { hash, is_leaf }
    }
}

/// [`Children`] is just a collection of children belonging to a [`InternalNode`], indexed from 0 to
/// 15, inclusive.
pub(crate) type Children = HashMap<Nibble, Child>;

/// Represents a 4-level subtree with 16 children at the bottom level. Theoretically, this reduces
/// IOPS to query a tree by 4x since we compress 4 levels in a standard Merkle tree into 1 node.
/// Though we choose the same internal node structure as that of Patricia Merkle tree, the root hash
/// computation logic is similar to a 4-level sparse Merkle tree except for some customizations. See
/// the `CryptoHash` trait implementation below for details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InternalNode {
    // Up to 16 children.
    children: Children,
    //Node's hash cache
    cached_hash: Cell<Option<SMTNodeHash>>,
}

/// Computes the hash of internal node according to [`JellyfishTree`](super::JellyfishTree)
/// data structure in the logical view. `start` and `nibble_height` determine a subtree whose
/// root hash we want to get. For an internal node with 16 children at the bottom level, we compute
/// the root hash of it as if a full binary Merkle tree with 16 leaves as below:
///
/// ```text
///   4 ->              +------ root hash ------+
///                     |                       |
///   3 ->        +---- # ----+           +---- # ----+
///               |           |           |           |
///   2 ->        #           #           #           #
///             /   \       /   \       /   \       /   \
///   1 ->     #     #     #     #     #     #     #     #
///           / \   / \   / \   / \   / \   / \   / \   / \
///   0 ->   0   1 2   3 4   5 6   7 8   9 A   B C   D E   F
///   ^
/// height
/// ```
///
/// As illustrated above, at nibble height 0, `0..F` in hex denote 16 chidren hashes.  Each `#`
/// means the hash of its two direct children, which will be used to generate the hash of its
/// parent with the hash of its sibling. Finally, we can get the hash of this internal node.
///
/// However, if an internal node doesn't have all 16 chidren exist at height 0 but just a few of
/// them, we have a modified hashing rule on top of what is stated above:
/// 1. From top to bottom, a node will be replaced by a leaf child if the subtree rooted at this
///     node has only one child at height 0 and it is a leaf child.
/// 2. From top to bottom, a node will be replaced by the placeholder node if the subtree rooted at
///     this node doesn't have any child at height 0. For example, if an internal node has 3 leaf
///     children at index 0, 3, 8, respectively, and 1 internal node at index C, then the computation
///     graph will be like:
///
/// ```text
///   4 ->              +------ root hash ------+
///                     |                       |
///   3 ->        +---- # ----+           +---- # ----+
///               |           |           |           |
///   2 ->        #           @           8           #
///             /   \                               /   \
///   1 ->     0     3                             #     @
///                                               / \
///   0 ->                                       C   @
///   ^
/// height
/// Note: @ denotes placeholder hash.
/// ```
impl SMTHash for InternalNode {
    fn merkle_hash(&self) -> SMTNodeHash {
        match self.cached_hash.get() {
            Some(hash) => hash,
            None => {
                let hash = self.make_hash(
                    0,  // start index
                    16, // the number of leaves in the subtree of which we want the hash of root
                    self.generate_bitmaps(),
                );
                self.cached_hash.set(Some(hash));
                hash
            }
        }
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl Arbitrary for InternalNode {
    type Parameters = ();
    fn arbitrary_with(_args: ()) -> Self::Strategy {
        hash_map(any::<Nibble>(), any::<Child>(), 1..=16)
            .prop_filter(
                "InternalNode constructor panics when its only child is a leaf.",
                |children| {
                    !(children.len() == 1 && children.values().next().expect("Must exist.").is_leaf)
                },
            )
            .prop_map(InternalNode::new)
            .boxed()
    }

    type Strategy = BoxedStrategy<Self>;
}

impl InternalNode {
    /// Creates a new Internal node.
    pub fn new(children: Children) -> Self {
        // Assert the internal node must have >= 1 children. If it only has one child, it cannot be
        // a leaf node. Otherwise, the leaf node should be a child of this internal node's parent.
        assert!(!children.is_empty());
        if children.len() == 1 {
            assert!(
                !children
                    .values()
                    .next()
                    .expect("Must have 1 element")
                    .is_leaf
            )
        }
        Self {
            children,
            cached_hash: Cell::new(None),
        }
    }

    pub fn serialize(&self, binary: &mut Vec<u8>) -> Result<()> {
        let (mut existence_bitmap, leaf_bitmap) = self.generate_bitmaps();
        binary.write_u16::<LittleEndian>(existence_bitmap)?;
        binary.write_u16::<LittleEndian>(leaf_bitmap)?;
        for _ in 0..existence_bitmap.count_ones() {
            let next_child = existence_bitmap.trailing_zeros() as u8;
            let child = &self.children[&Nibble::from(next_child)];
            // serialize_u64_varint(child.version, binary);
            binary.extend(child.hash.to_vec());
            existence_bitmap &= !(1 << next_child);
        }
        Ok(())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(data);
        let len = data.len();

        // Read and validate existence and leaf bitmaps
        let mut existence_bitmap = reader.read_u16::<LittleEndian>()?;
        let leaf_bitmap = reader.read_u16::<LittleEndian>()?;
        match existence_bitmap {
            0 => return Err(NodeDecodeError::NoChildren.into()),
            _ if (existence_bitmap & leaf_bitmap) != leaf_bitmap => {
                return Err(NodeDecodeError::ExtraLeaves {
                    existing: existence_bitmap,
                    leaves: leaf_bitmap,
                }
                .into());
            }
            _ => (),
        }

        // Reconstruct children
        let mut children = HashMap::new();
        for _ in 0..existence_bitmap.count_ones() {
            let next_child = existence_bitmap.trailing_zeros() as u8;
            // let version = deserialize_u64_varint(&mut reader)?;
            let pos = reader.position() as usize;
            let remaining = len - pos;
            ensure!(
                remaining >= size_of::<SMTNodeHash>(),
                "not enough bytes left, children: {}, bytes: {}",
                existence_bitmap.count_ones(),
                remaining
            );
            let child_bit = 1 << next_child;
            children.insert(
                Nibble::from(next_child),
                Child::new(
                    SMTNodeHash::from_slice(
                        &reader.get_ref()[pos..pos + size_of::<SMTNodeHash>()],
                    )?,
                    // version,
                    (leaf_bitmap & child_bit) != 0,
                ),
            );
            reader.seek(SeekFrom::Current(size_of::<SMTNodeHash>() as i64))?;
            existence_bitmap &= !child_bit;
        }
        assert_eq!(existence_bitmap, 0);
        Ok(Self::new(children))
    }

    /// Gets the `n`-th child.
    pub fn child(&self, n: Nibble) -> Option<&Child> {
        self.children.get(&n)
    }

    /// Return the total number of existing children.
    pub fn num_children(&self) -> usize {
        self.children.len()
    }

    /// Generates `existence_bitmap` and `leaf_bitmap` as a pair of `u16`s: child at index `i`
    /// exists if `existence_bitmap[i]` is set; child at index `i` is leaf node if
    /// `leaf_bitmap[i]` is set.
    pub fn generate_bitmaps(&self) -> (u16, u16) {
        let mut existence_bitmap = 0;
        let mut leaf_bitmap = 0;
        for (nibble, child) in self.children.iter() {
            let i = u8::from(*nibble);
            existence_bitmap |= 1u16 << i;
            leaf_bitmap |= (child.is_leaf as u16) << i;
        }
        // `leaf_bitmap` must be a subset of `existence_bitmap`.
        assert_eq!(existence_bitmap | leaf_bitmap, existence_bitmap);
        (existence_bitmap, leaf_bitmap)
    }

    /// Given a range [start, start + width), returns the sub-bitmap of that range.
    fn range_bitmaps(start: u8, width: u8, bitmaps: (u16, u16)) -> (u16, u16) {
        assert!(start < 16 && width.count_ones() == 1 && start % width == 0);
        // A range with `start == 8` and `width == 4` will generate a mask 0b0000111100000000.
        let mask = if width == 16 {
            0xffff
        } else {
            assert!(width <= 16);
            (1 << width) - 1
        } << start;
        (bitmaps.0 & mask, bitmaps.1 & mask)
    }

    fn make_hash(
        &self,
        start: u8,
        width: u8,
        (existence_bitmap, leaf_bitmap): (u16, u16),
    ) -> SMTNodeHash {
        // Given a bit [start, 1 << nibble_height], return the value of that range.
        let (range_existence_bitmap, range_leaf_bitmap) =
            Self::range_bitmaps(start, width, (existence_bitmap, leaf_bitmap));
        if range_existence_bitmap == 0 {
            // No child under this subtree
            *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE
        } else if range_existence_bitmap.count_ones() == 1 && (range_leaf_bitmap != 0 || width == 1)
        {
            // Only 1 leaf child under this subtree or reach the lowest level
            let only_child_index = Nibble::from(range_existence_bitmap.trailing_zeros() as u8);
            self.child(only_child_index)
                .with_context(|| {
                    format!(
                        "Corrupted internal node: existence_bitmap indicates \
                         the existence of a non-exist child at index {:x}",
                        only_child_index
                    )
                })
                .unwrap()
                .hash
        } else {
            let left_child = self.make_hash(start, width / 2, (existence_bitmap, leaf_bitmap));
            let right_child = self.make_hash(
                start + width / 2,
                width / 2,
                (existence_bitmap, leaf_bitmap),
            );
            SparseMerkleInternalNode::new(left_child, right_child).merkle_hash()
        }
    }

    /// Gets the child and its corresponding siblings that are necessary to generate the proof for
    /// the `n`-th child. If it is an existence proof, the returned child must be the `n`-th
    /// child; otherwise, the returned child may be another child. See inline explanation for
    /// details. When calling this function with n = 11 (node `b` in the following graph), the
    /// range at each level is illustrated as a pair of square brackets:
    ///
    /// ```text
    ///     4      [f   e   d   c   b   a   9   8   7   6   5   4   3   2   1   0] -> root level
    ///            ---------------------------------------------------------------
    ///     3      [f   e   d   c   b   a   9   8] [7   6   5   4   3   2   1   0] width = 8
    ///                                  chs <--┘                        shs <--┘
    ///     2      [f   e   d   c] [b   a   9   8] [7   6   5   4] [3   2   1   0] width = 4
    ///                  shs <--┘               └--> chs
    ///     1      [f   e] [d   c] [b   a] [9   8] [7   6] [5   4] [3   2] [1   0] width = 2
    ///                          chs <--┘       └--> shs
    ///     0      [f] [e] [d] [c] [b] [a] [9] [8] [7] [6] [5] [4] [3] [2] [1] [0] width = 1
    ///     ^                chs <--┘   └--> shs
    ///     |   MSB|<---------------------- uint 16 ---------------------------->|LSB
    ///  height    chs: `child_half_start`         shs: `sibling_half_start`
    /// ```
    pub fn get_child_with_siblings(&self, n: Nibble) -> (Option<NodeKey>, Vec<SMTNodeHash>) {
        let mut siblings = vec![];
        let (existence_bitmap, leaf_bitmap) = self.generate_bitmaps();

        // Nibble height from 3 to 0.
        for h in (0..4).rev() {
            // Get the number of children of the internal node that each subtree at this height
            // covers.
            let width = 1 << h;
            let (child_half_start, sibling_half_start) = get_child_and_sibling_half_start(n, h);
            // Compute the root hash of the subtree rooted at the sibling of `r`.
            siblings.push(self.make_hash(
                sibling_half_start,
                width,
                (existence_bitmap, leaf_bitmap),
            ));

            let (range_existence_bitmap, range_leaf_bitmap) =
                Self::range_bitmaps(child_half_start, width, (existence_bitmap, leaf_bitmap));

            if range_existence_bitmap == 0 {
                // No child in this range.
                return (None, siblings);
            } else if range_existence_bitmap.count_ones() == 1
                && (range_leaf_bitmap.count_ones() == 1 || width == 1)
            {
                // Return the only 1 leaf child under this subtree or reach the lowest level
                // Even this leaf child is not the n-th child, it should be returned instead of
                // `None` because it's existence indirectly proves the n-th child doesn't exist.
                // Please read proof format for details.
                let only_child_index = Nibble::from(range_existence_bitmap.trailing_zeros() as u8);
                return (
                    {
                        let only_child = self
                            .child(only_child_index)
                            // Should be guaranteed by the self invariants, but these are not easy to express at the moment
                            .with_context(|| {
                                format!(
                                    "Corrupted internal node: child_bitmap indicates \
                                     the existence of a non-exist child at index {:x}",
                                    only_child_index
                                )
                            })
                            .unwrap();
                        Some(only_child.hash)
                    },
                    siblings,
                );
            }
        }
        unreachable!("Impossible to get here without returning even at the lowest level.")
    }

    /// Get all child hash
    pub fn all_child(&self) -> Vec<SMTNodeHash> {
        self.children.values().map(|c| c.hash).collect()
    }
}

/// Given a nibble, computes the start position of its `child_half_start` and `sibling_half_start`
/// at `height` level.
pub(crate) fn get_child_and_sibling_half_start(n: Nibble, height: u8) -> (u8, u8) {
    // Get the index of the first child belonging to the same subtree whose root, let's say `r` is
    // at `height` that the n-th child belongs to.
    // Note: `child_half_start` will be always equal to `n` at height 0.
    let child_half_start = (0xff << height) & u8::from(n);

    // Get the index of the first child belonging to the subtree whose root is the sibling of `r`
    // at `height`.
    let sibling_half_start = child_half_start ^ (1 << height);

    (child_half_start, sibling_half_start)
}

/// Represents an account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LeafNode<K, V> {
    /// The origin key associated with this leaf node's Value.
    key: K,
    /// The blob value associated with `key`.
    value: SMTObject<V>,
    cached_hash: Cell<Option<SMTNodeHash>>,
}

impl<K, V> LeafNode<K, V>
where
    K: Key,
    V: Value,
{
    /// Creates a new leaf node.
    pub fn new<NV: Into<SMTObject<V>>>(key: K, value: NV) -> Self {
        Self {
            key,
            value: value.into(),
            cached_hash: Cell::new(None),
        }
    }

    pub fn cached_hash(&self) -> SMTNodeHash {
        match self.cached_hash.get() {
            Some(hash) => hash,
            None => {
                let hash = self.merkle_hash();
                self.cached_hash.set(Some(hash));
                hash
            }
        }
    }

    /// Gets the key
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Gets the hash of origin key.
    pub fn key_hash(&self) -> SMTNodeHash {
        self.key.merkle_hash()
    }

    /// Gets the hash of associated blob.
    pub fn value_hash(&self) -> SMTNodeHash {
        self.value.merkle_hash()
    }

    /// Gets the associated blob itself.
    pub fn value(&self) -> &SMTObject<V> {
        &self.value
    }

    pub fn serialize(&self, binary: &mut Vec<u8>) -> Result<()> {
        binary.extend(bcs::to_bytes(self)?);
        Ok(())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bcs::from_bytes(data).map_err(|e| e.into())
    }

    pub fn into(self) -> (K, SMTObject<V>) {
        (self.key, self.value)
    }
}

#[derive(Serialize, Deserialize)]
struct RawKV {
    key: H256,
    value: Vec<u8>,
}

impl<K, V> Serialize for LeafNode<K, V>
where
    K: Key,
    V: Value,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let wrapper = RawKV {
            key: self.key.into(),
            value: self.value.raw.clone(),
        };
        wrapper.serialize(serializer)
    }
}

impl<'de, K, V> Deserialize<'de> for LeafNode<K, V>
where
    K: Key,
    V: Value,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wrapper = RawKV::deserialize(deserializer)?;
        Ok(LeafNode::new(
            K::from(wrapper.key),
            V::from_raw(wrapper.value).map_err(serde::de::Error::custom)?,
        ))
    }
}

/// Computes the hash of a [`LeafNode`].
impl<K, V> SMTHash for LeafNode<K, V>
where
    K: Key,
    V: Value,
{
    fn merkle_hash(&self) -> SMTNodeHash {
        SparseMerkleLeafNode::new(self.key.merkle_hash(), self.value.merkle_hash()).merkle_hash()
    }
}

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive)]
enum NodeTag {
    Null = 0,
    Internal = 1,
    Leaf = 2,
}

/// The concrete node type of [`JellyfishMerkleTree`](super::JellyfishMerkleTree).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Node<K, V> {
    /// Represents `null`.
    Null,
    /// A wrapper of [`InternalNode`].
    Internal(InternalNode),
    /// A wrapper of [`LeafNode`].
    Leaf(LeafNode<K, V>),
}

impl<K, V> From<InternalNode> for Node<K, V> {
    fn from(node: InternalNode) -> Self {
        Node::Internal(node)
    }
}

impl From<InternalNode> for Children {
    fn from(node: InternalNode) -> Self {
        node.children
    }
}

impl<K, V> From<LeafNode<K, V>> for Node<K, V> {
    fn from(node: LeafNode<K, V>) -> Self {
        Node::Leaf(node)
    }
}

impl<K, V> Node<K, V>
where
    K: Key,
    V: Value,
{
    /// Creates the [`Null`](Node::Null) variant.
    pub fn new_null() -> Self {
        Node::Null
    }

    /// Creates the [`Internal`](Node::Internal) variant.
    pub fn new_internal(children: Children) -> Self {
        Node::Internal(InternalNode::new(children))
    }

    /// Creates the [`Leaf`](Node::Leaf) variant.
    pub fn new_leaf<NV: Into<SMTObject<V>>>(key: K, value: NV) -> Self {
        Node::Leaf(LeafNode::new(key, value))
    }

    /// Returns `true` if the node is a leaf node.
    pub fn is_leaf(&self) -> bool {
        matches!(self, Node::Leaf(_))
    }

    /// Serializes to bytes for physical storage.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = vec![];
        match self {
            Node::Null => {
                out.push(NodeTag::Null as u8);
            }
            Node::Internal(internal_node) => {
                out.push(NodeTag::Internal as u8);
                internal_node.serialize(&mut out)?;
            }
            Node::Leaf(leaf_node) => {
                out.push(NodeTag::Leaf as u8);
                leaf_node.serialize(&mut out)?;
            }
        }
        Ok(out)
    }

    /// Recovers from serialized bytes in physical storage.
    pub fn decode(val: &[u8]) -> Result<Node<K, V>> {
        if val.is_empty() {
            return Err(NodeDecodeError::EmptyInput.into());
        }
        let tag = val[0];
        let node_tag = NodeTag::from_u8(tag);
        match node_tag {
            Some(NodeTag::Null) => Ok(Node::Null),
            Some(NodeTag::Internal) => Ok(Node::Internal(InternalNode::deserialize(&val[1..])?)),
            Some(NodeTag::Leaf) => Ok(Node::Leaf(LeafNode::deserialize(&val[1..])?)),
            None => Err(NodeDecodeError::UnknownTag { unknown_tag: tag }.into()),
        }
    }
}

impl<K, V> SMTHash for Node<K, V>
where
    K: Key,
    V: Value,
{
    fn merkle_hash(&self) -> SMTNodeHash {
        match self {
            Node::Null => *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
            Node::Internal(internal_node) => internal_node.merkle_hash(),
            Node::Leaf(leaf_node) => leaf_node.merkle_hash(),
        }
    }
}

impl<K, V> Serialize for Node<K, V>
where
    K: Key,
    V: Value,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.encode().map_err(serde::ser::Error::custom)?.as_slice())
    }
}

impl<'de, K, V> Deserialize<'de> for Node<K, V>
where
    K: Key,
    V: Value,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Node::decode(bytes.as_slice()).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SparseMerkleInternalNode {
    left_child: SMTNodeHash,
    right_child: SMTNodeHash,
}

impl SparseMerkleInternalNode {
    pub fn new(left_child: SMTNodeHash, right_child: SMTNodeHash) -> Self {
        Self {
            left_child,
            right_child,
        }
    }
}

impl SMTHash for SparseMerkleInternalNode {
    fn merkle_hash(&self) -> SMTNodeHash {
        SMTNodeHash::from_node_hashes(self.left_child, self.right_child)
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct SparseMerkleLeafNode {
    pub key_hash: SMTNodeHash,
    pub value_hash: SMTNodeHash,
}

impl SparseMerkleLeafNode {
    pub fn new(key_hash: SMTNodeHash, value_hash: SMTNodeHash) -> Self {
        SparseMerkleLeafNode {
            key_hash,
            value_hash,
        }
    }
}

impl SMTHash for SparseMerkleLeafNode {
    fn merkle_hash(&self) -> SMTNodeHash {
        SMTNodeHash::from_node_hashes(self.key_hash, self.value_hash)
    }
}

/// Error thrown when a [`Node`] fails to be deserialized out of a byte sequence stored in physical
/// storage, via [`Node::decode`].
#[derive(Debug, Error, Eq, PartialEq)]
pub enum NodeDecodeError {
    /// Input is empty.
    #[error("Missing tag due to empty input")]
    EmptyInput,

    /// The first byte of the input is not a known tag representing one of the variants.
    #[error("lead tag byte is unknown: {}", unknown_tag)]
    UnknownTag { unknown_tag: u8 },

    /// No children found in internal node
    #[error("No children found in internal node")]
    NoChildren,

    /// Extra leaf bits set
    #[error(
        "Non-existent leaf bits set, existing: {}, leaves: {}",
        existing,
        leaves
    )]
    ExtraLeaves { existing: u16, leaves: u16 },
}

/// Helper function to serialize version in a more efficient encoding.
/// We use a super simple encoding - the high bit is set if more bytes follow.
fn serialize_u64_varint(mut num: u64, binary: &mut Vec<u8>) {
    for _ in 0..8 {
        let low_bits = num as u8 & 0x7f;
        num >>= 7;
        let more = (num > 0) as u8;
        binary.push(low_bits | more << 7);
        if more == 0 {
            return;
        }
    }
    // Last byte is encoded raw; this means there are no bad encodings.
    assert_ne!(num, 0);
    assert!(num <= 0xff);
    binary.push(num as u8);
}

/// Helper function to deserialize versions from above encoding.
fn deserialize_u64_varint<T>(reader: &mut T) -> Result<u64>
where
    T: Read,
{
    let mut num = 0u64;
    for i in 0..8 {
        let byte = reader.read_u8()?;
        let more = (byte & 0x80) != 0;
        num |= u64::from(byte & 0x7f) << (i * 7);
        if !more {
            return Ok(num);
        }
    }
    // Last byte is encoded as is.
    let byte = reader.read_u8()?;
    num |= u64::from(byte) << 56;
    Ok(num)
}
