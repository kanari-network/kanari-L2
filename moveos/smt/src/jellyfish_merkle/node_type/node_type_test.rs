// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::super::hash::{SMTNodeHash, SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE};
use super::super::nibble_path::NibblePath;
use super::*;
use crate::{
    EncodeToObject,
    jellyfish_merkle::mock_tree_store::{TestKey, TestValue},
};
use proptest::prelude::*;
use std::{panic, rc::Rc};

fn hash_internal(left: SMTNodeHash, right: SMTNodeHash) -> SMTNodeHash {
    SparseMerkleInternalNode::new(left, right).merkle_hash()
}

fn hash_leaf(key: SMTNodeHash, value_hash: SMTNodeHash) -> SMTNodeHash {
    SparseMerkleLeafNode::new(key, value_hash).merkle_hash()
}

// Generate a random node key with 63 nibbles.
fn random_63nibblepath() -> NibblePath {
    let hash = SMTNodeHash::random();
    let mut bytes = hash.to_vec();
    *bytes.last_mut().unwrap() &= 0xf0;
    NibblePath::new_odd(bytes)
}

// Generate a pair of leaf node key and account key with a passed-in 63-nibble node key and the last
// nibble to be appended.
fn gen_leaf_keys(nibble_path: &NibblePath, nibble: Nibble) -> TestKey {
    assert_eq!(nibble_path.num_nibbles(), 63);
    let mut np = nibble_path.clone();
    np.push(nibble);
    TestKey(SMTNodeHash::from_slice(np.bytes()).unwrap())
}

#[test]
fn test_encode_decode() {
    let nibble_path = random_63nibblepath();
    // let nibble_path = NibblePath::new(vec![]);

    let leaf1_keys = gen_leaf_keys(&nibble_path, Nibble::from(1));
    let leaf1_node: Node<TestKey, TestValue> =
        Node::new_leaf(leaf1_keys, TestValue::from(vec![0x00]));
    let leaf2_keys = gen_leaf_keys(&nibble_path, Nibble::from(2));
    let leaf2_node: Node<TestKey, TestValue> =
        Node::new_leaf(leaf2_keys, TestValue::from(vec![0x01]));

    let mut children = Children::default();
    children.insert(Nibble::from(1), Child::new(leaf1_node.merkle_hash(), true));
    children.insert(Nibble::from(2), Child::new(leaf2_node.merkle_hash(), true));

    let account_key = TestKey(SMTNodeHash::random());
    let nodes = vec![
        Node::new_internal(children),
        Node::new_leaf(account_key, TestValue::from(vec![0x02])),
    ];
    for n in &nodes {
        let v = n.encode().unwrap();
        assert_eq!(*n, Node::decode(&v).unwrap());
    }
    // Error cases
    if let Err(e) = Node::<TestKey, TestValue>::decode(&[]) {
        assert_eq!(
            e.downcast::<NodeDecodeError>().unwrap(),
            NodeDecodeError::EmptyInput
        );
    }
    if let Err(e) = Node::<TestKey, TestValue>::decode(&[100]) {
        assert_eq!(
            e.downcast::<NodeDecodeError>().unwrap(),
            NodeDecodeError::UnknownTag { unknown_tag: 100 }
        );
    }
}

proptest! {
    #[test]
    fn test_u64_varint_roundtrip(input in any::<u64>()) {
        let mut vec = vec![];
        serialize_u64_varint(input, &mut vec);
        assert_eq!(deserialize_u64_varint(&mut Cursor::new(vec)).unwrap(), input);
    }

    #[test]
    fn test_internal_node_roundtrip(input in any::<InternalNode>()) {
        let mut vec = vec![];
        input.serialize(&mut vec).unwrap();
        assert_eq!(InternalNode::deserialize(&vec).unwrap(), input);
    }
}

#[test]
fn test_internal_validity() {
    let result = panic::catch_unwind(|| {
        let children = Children::default();
        InternalNode::new(children)
    });
    assert!(result.is_err());

    let result = panic::catch_unwind(|| {
        let mut children = Children::default();
        children.insert(
            Nibble::from(1),
            Child::new(SMTNodeHash::random(), true /* is_leaf */),
        );
        InternalNode::new(children);
    });
    assert!(result.is_err());
}

#[test]
fn test_leaf_hash() {
    {
        let key = TestKey::random();
        let blob = TestValue::from(vec![0x02]).into_object().unwrap();
        let value_hash = blob.merkle_hash();
        let hash = hash_leaf(key.merkle_hash(), value_hash);
        let leaf_node: Node<TestKey, TestValue> = Node::new_leaf(key, blob);
        assert_eq!(leaf_node.merkle_hash(), hash);
    }
}

proptest! {
    #[test]
    fn two_leaves_test1(index1 in (0..8u8).prop_map(Nibble::from), index2 in (8..16u8).prop_map(Nibble::from)) {
        let nibble_path = random_63nibblepath();
        let mut children = Children::default();

        let leaf1_node_key = gen_leaf_keys( &nibble_path, index1);
        let leaf2_node_key = gen_leaf_keys( &nibble_path, index2);
        let hash1 = leaf1_node_key.0;
        let hash2 = leaf2_node_key.0;

        children.insert(index1, Child::new(hash1,  true));
        children.insert(index2, Child::new(hash2,  true));
        let internal_node = InternalNode::new(children);

        // Internal node will have a structure below
        //
        //              root
        //              / \
        //             /   \
        //        leaf1     leaf2
        //
        let root_hash = hash_internal(hash1, hash2);
        prop_assert_eq!(internal_node.merkle_hash(), root_hash);

        for i in 0..8 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf1_node_key.0), vec![hash2])
            );
        }
        for i in 8..16 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf2_node_key.0), vec![hash1])
            );
        }

    }

    #[test]
    fn two_leaves_test2(index1 in (4..6u8).prop_map(Nibble::from), index2 in (6..8u8).prop_map(Nibble::from)) {
        let nibble_path = random_63nibblepath();
        let mut children = Children::default();

        let leaf1_node_key = gen_leaf_keys( &nibble_path, index1);
        let leaf2_node_key = gen_leaf_keys( &nibble_path, index2);
        let hash1 = leaf1_node_key.0;
        let hash2 = leaf2_node_key.0;

        children.insert(index1, Child::new(hash1,  true));
        children.insert(index2, Child::new(hash2,  true));
        let internal_node = InternalNode::new(children);

        // Internal node will have a structure below
        //
        //              root
        //              /
        //             /
        //            x2
        //             \
        //              \
        //               x1
        //              / \
        //             /   \
        //        leaf1     leaf2
        let hash_x1 = hash_internal(hash1, hash2);
        let hash_x2 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x1);

        let root_hash = hash_internal(hash_x2, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        assert_eq!(internal_node.merkle_hash(), root_hash);

        for i in 0..4 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (None, vec![*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x1])
            );
        }

        for i in 4..6 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (
                    Some(leaf1_node_key.0),
                    vec![
                        *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                        *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                        hash2
                    ]
                )
            );
        }

        for i in 6..8 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (
                    Some(leaf2_node_key.0),
                    vec![
                        *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                        *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                        hash1
                    ]
                )
            );
        }

        for i in 8..16 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (None, vec![hash_x2])
            );
        }

    }

    #[test]
    fn three_leaves_test1(index1 in (0..4u8).prop_map(Nibble::from), index2 in (4..8u8).prop_map(Nibble::from), index3 in (8..16u8).prop_map(Nibble::from)) {
        let nibble_path = random_63nibblepath();
        let mut children = Children::default();

        let leaf1_node_key = gen_leaf_keys( &nibble_path, index1);
        let leaf2_node_key = gen_leaf_keys( &nibble_path, index2);
        let leaf3_node_key = gen_leaf_keys( &nibble_path, index3);

        let hash1 = leaf1_node_key.0;
        let hash2 = leaf2_node_key.0;
        let hash3 = leaf3_node_key.0;

        children.insert(index1, Child::new(hash1, true));
        children.insert(index2, Child::new(hash2,  true));
        children.insert(index3, Child::new(hash3,  true));
        let internal_node = InternalNode::new(children);
        // Internal node will have a structure below
        //
        //               root
        //               / \
        //              /   \
        //             x     leaf3
        //            / \
        //           /   \
        //      leaf1     leaf2
        let hash_x = hash_internal(hash1, hash2);
        let root_hash = hash_internal(hash_x, hash3);
        prop_assert_eq!(internal_node.merkle_hash(), root_hash);

        for i in 0..4 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf1_node_key.0),vec![hash3, hash2])
            );
        }

        for i in 4..8 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf2_node_key.0),vec![hash3, hash1])
            );
        }

        for i in 8..16 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf3_node_key.0),vec![hash_x])
            );
        }
    }

    #[test]
    fn mixed_nodes_test(index1 in (0..2u8).prop_map(Nibble::from), index2 in (8..16u8).prop_map(Nibble::from)) {
        let nibble_path = random_63nibblepath();
        let mut children = Children::default();

        let leaf1_node_key = gen_leaf_keys(&nibble_path, index1);
        let internal2_node_key = gen_leaf_keys(&nibble_path, 2.into());
        let internal3_node_key = gen_leaf_keys(&nibble_path, 7.into());
        let leaf4_node_key = gen_leaf_keys( &nibble_path, index2);

        let hash1 = leaf1_node_key.0;
        let hash2 = internal2_node_key.0;
        let hash3 = internal3_node_key.0;
        let hash4 = leaf4_node_key.0;
        children.insert(index1, Child::new(hash1,  true));
        children.insert(2.into(), Child::new(hash2,  false));
        children.insert(7.into(), Child::new(hash3, false));
        children.insert(index2, Child::new(hash4, true));
        let internal_node = InternalNode::new(children);
        // Internal node (B) will have a structure below
        //
        //                   B (root hash)
        //                  / \
        //                 /   \
        //                x5    leaf4
        //               / \
        //              /   \
        //             x2    x4
        //            / \     \
        //           /   \     \
        //      leaf1    x1     x3
        //               /       \
        //              /         \
        //          internal2      internal3
        //
        let hash_x1 = hash_internal(hash2, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        let hash_x2 = hash_internal(hash1, hash_x1);
        let hash_x3 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash3);
        let hash_x4 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x3);
        let hash_x5 = hash_internal(hash_x2, hash_x4);
        let root_hash = hash_internal(hash_x5, hash4);
        assert_eq!(internal_node.merkle_hash(), root_hash);

        for i in 0..2 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (
                    Some(leaf1_node_key.0),
                    vec![hash4, hash_x4, hash_x1]
                )
            );
        }

        prop_assert_eq!(
                internal_node.get_child_with_siblings( 2.into()),
            (
                Some(internal2_node_key.0),
                vec![
                    hash4,
                    hash_x4,
                    hash1,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                ]
            )
        );

        prop_assert_eq!(
                internal_node.get_child_with_siblings( 3.into()),

            (
                None,
                vec![hash4, hash_x4, hash1, hash2,]
            )
        );

        for i in 4..6 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (
                    None,
                    vec![hash4, hash_x2, hash_x3]
                )
            );
        }

        prop_assert_eq!(
                internal_node.get_child_with_siblings( 6.into()),
            (
                None,
                vec![
                    hash4,
                    hash_x2,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash3,
                ]
            )
        );

        prop_assert_eq!(
                internal_node.get_child_with_siblings( 7.into()),
            (
                Some(internal3_node_key.0),
                vec![
                    hash4,
                    hash_x2,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                ]
            )
        );

        for i in 8..16 {
            prop_assert_eq!(
                internal_node.get_child_with_siblings( i.into()),
                (Some(leaf4_node_key.0), vec![hash_x5])
            );
        }
    }
}

#[test]
fn test_internal_hash_and_proof() {
    // non-leaf case 1
    {
        let mut children = Children::default();

        let index1 = Nibble::from(4);
        let index2 = Nibble::from(15);
        let hash1 = SMTNodeHash::random();
        let hash2 = SMTNodeHash::random();
        children.insert(index1, Child::new(hash1, false));
        children.insert(index2, Child::new(hash2, false));
        let internal_node = InternalNode::new(children);
        // Internal node (B) will have a structure below
        //
        //              root
        //              / \
        //             /   \
        //            x3    x6
        //             \     \
        //              \     \
        //              x2     x5
        //              /       \
        //             /         \
        //            x1          x4
        //           /             \
        //          /               \
        // non-leaf1             non-leaf2
        //
        let hash_x1 = hash_internal(hash1, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        let hash_x2 = hash_internal(hash_x1, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        let hash_x3 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x2);
        let hash_x4 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash2);
        let hash_x5 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x4);
        let hash_x6 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x5);
        let root_hash = hash_internal(hash_x3, hash_x6);
        assert_eq!(internal_node.merkle_hash(), root_hash);

        for i in 0..4 {
            let result = internal_node.get_child_with_siblings(i.into());
            assert_eq!(result, (None, vec![hash_x6, hash_x2]));
        }

        assert_eq!(
            internal_node.get_child_with_siblings(5.into()),
            (
                None,
                vec![
                    hash_x6,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash1
                ]
            )
        );
        for i in 6..8 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (
                    None,
                    vec![hash_x6, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x1]
                )
            );
        }

        for i in 8..12 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (None, vec![hash_x3, hash_x5])
            );
        }

        for i in 12..14 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (
                    None,
                    vec![hash_x3, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x4]
                )
            );
        }
        assert_eq!(
            internal_node.get_child_with_siblings(14.into()),
            (
                None,
                vec![
                    hash_x3,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash2
                ]
            )
        );
    }

    // non-leaf case 2
    {
        let mut children = Children::default();

        let index1 = Nibble::from(0);
        let index2 = Nibble::from(7);
        let hash1 = SMTNodeHash::random();
        let hash2 = SMTNodeHash::random();

        children.insert(index1, Child::new(hash1, false));
        children.insert(index2, Child::new(hash2, false));
        let internal_node = InternalNode::new(children);
        // Internal node will have a structure below
        //
        //                     root
        //                     /
        //                    /
        //                   x5
        //                  / \
        //                 /   \
        //               x2     x4
        //               /       \
        //              /         \
        //            x1           x3
        //            /             \
        //           /               \
        //  non-leaf1                 non-leaf2

        let hash_x1 = hash_internal(hash1, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        let hash_x2 = hash_internal(hash_x1, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        let hash_x3 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash2);
        let hash_x4 = hash_internal(*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x3);
        let hash_x5 = hash_internal(hash_x2, hash_x4);
        let root_hash = hash_internal(hash_x5, *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE);
        assert_eq!(internal_node.merkle_hash(), root_hash);

        assert_eq!(
            internal_node.get_child_with_siblings(1.into()),
            (
                None,
                vec![
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash_x4,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash1,
                ]
            )
        );

        for i in 2..4 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (
                    None,
                    vec![*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x4, hash_x1]
                )
            );
        }

        for i in 4..6 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (
                    None,
                    vec![*SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE, hash_x2, hash_x3]
                )
            );
        }

        assert_eq!(
            internal_node.get_child_with_siblings(6.into()),
            (
                None,
                vec![
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash_x2,
                    *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
                    hash2
                ]
            )
        );

        for i in 8..16 {
            assert_eq!(
                internal_node.get_child_with_siblings(i.into()),
                (None, vec![hash_x5])
            );
        }
    }
}

enum BinaryTreeNode {
    Internal(BinaryTreeInternalNode),
    Child(BinaryTreeChildNode),
    Null,
}

impl BinaryTreeNode {
    fn new_child(index: u8, child: &Child) -> Self {
        Self::Child(BinaryTreeChildNode {
            index,
            hash: child.hash,
            is_leaf: child.is_leaf,
        })
    }

    fn new_internal(
        first_child_index: u8,
        num_children: u8,
        left: BinaryTreeNode,
        right: BinaryTreeNode,
    ) -> Self {
        let hash =
            SparseMerkleInternalNode::new(left.merkle_hash(), right.merkle_hash()).merkle_hash();

        Self::Internal(BinaryTreeInternalNode {
            begin: first_child_index,
            width: num_children,
            left: Rc::new(left),
            right: Rc::new(right),
            hash,
        })
    }

    fn hash(&self) -> SMTNodeHash {
        match self {
            BinaryTreeNode::Internal(node) => node.hash,
            BinaryTreeNode::Child(node) => node.hash,
            BinaryTreeNode::Null => *SPARSE_MERKLE_PLACEHOLDER_HASH_VALUE,
        }
    }
}

impl SMTHash for BinaryTreeNode {
    fn merkle_hash(&self) -> SMTNodeHash {
        self.hash()
    }
}

/// An internal node in a binary tree corresponding to a `InternalNode` being tested.
///
/// To describe its position in the binary tree, we use a range of level 0 (children level)
/// positions expressed by (`begin`, `width`)
///
/// For example, in the below graph, node A has (begin:0, width:4), while node B has
/// (begin:2, width: 2):
///
///                ...
///             /
///           [A]    ...
///         /    \
///        * [B]   ...
///       / \    / \
///      0   1  2   3    ... 15
struct BinaryTreeInternalNode {
    begin: u8,
    width: u8,
    left: Rc<BinaryTreeNode>,
    right: Rc<BinaryTreeNode>,
    hash: SMTNodeHash,
}

impl BinaryTreeInternalNode {
    fn in_left_subtree(&self, n: u8) -> bool {
        assert!(n >= self.begin);
        assert!(n < self.begin + self.width);

        n < self.begin + self.width / 2
    }
}

/// A child node, corresponding to one that is in the corresponding `InternalNode` being tested.
///
/// `index` is its key in `InternalNode::children`.
/// N.B. when `is_leaf` is true, in the binary tree represented by a `NaiveInternalNode`, the child
/// node will be brought up to the root of the highest subtree that has only that leaf.
#[derive(Clone, Copy)]
struct BinaryTreeChildNode {
    index: u8,
    hash: SMTNodeHash,
    is_leaf: bool,
}

struct NaiveInternalNode {
    root: Rc<BinaryTreeNode>,
}

impl NaiveInternalNode {
    fn from_clever_node(node: &InternalNode) -> Self {
        Self {
            root: Rc::new(Self::node_for_subtree(0, 16, &node.children)),
        }
    }

    fn node_for_subtree(begin: u8, width: u8, children: &Children) -> BinaryTreeNode {
        if width == 1 {
            return children
                .get(&begin.into())
                .map_or(BinaryTreeNode::Null, |child| {
                    BinaryTreeNode::new_child(begin, child)
                });
        }

        let half_width = width / 2;
        let left = Self::node_for_subtree(begin, half_width, children);
        let right = Self::node_for_subtree(begin + half_width, half_width, children);

        match (&left, &right) {
            (BinaryTreeNode::Null, BinaryTreeNode::Null) => {
                return BinaryTreeNode::Null;
            }
            (BinaryTreeNode::Null, BinaryTreeNode::Child(node))
            | (BinaryTreeNode::Child(node), BinaryTreeNode::Null) => {
                if node.is_leaf {
                    return BinaryTreeNode::Child(*node);
                }
            }
            _ => (),
        };

        BinaryTreeNode::new_internal(begin, width, left, right)
    }

    fn get_child_with_siblings(&self, n: u8) -> (Option<NodeKey>, Vec<SMTNodeHash>) {
        let mut current_node = Rc::clone(&self.root);
        let mut siblings = Vec::new();

        loop {
            match current_node.as_ref() {
                BinaryTreeNode::Internal(node) => {
                    if node.in_left_subtree(n) {
                        siblings.push(node.right.merkle_hash());
                        current_node = Rc::clone(&node.left);
                    } else {
                        siblings.push(node.left.merkle_hash());
                        current_node = Rc::clone(&node.right);
                    }
                }
                BinaryTreeNode::Child(node) => return (Some(node.hash), siblings),
                BinaryTreeNode::Null => return (None, siblings),
            }
        }
    }
}

proptest! {
    #[test]
    #[allow(clippy::unnecessary_operation)]
    fn test_get_child_with_siblings(
        node in any::<InternalNode>(),
    ) {
        for n in 0..16u8 {
            prop_assert_eq!(
                node.get_child_with_siblings(n.into()),
                NaiveInternalNode::from_clever_node(&node).get_child_with_siblings(n)
            )
        }
    }
}
