// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use moveos_types::h256::{ACCUMULATOR_PLACEHOLDER_HASH, H256, sha3_256_of};

use crate::{
    Accumulator, AccumulatorNode, AccumulatorTreeStore, LeafCount, MerkleAccumulator,
    node_index::NodeIndex, tree_store::mock::MockAccumulatorStore,
};
use std::time::SystemTime;
use std::{collections::HashMap, sync::Arc};

#[test]
fn test_get_leaves() {
    let leaves = create_leaves(1..100_000);
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );

    let _root_hash = accumulator.append(leaves.as_slice()).unwrap();
    let new_num_leaves = accumulator.num_leaves();
    let begin = SystemTime::now();
    (0..new_num_leaves).for_each(|idx| {
        let _leaf = accumulator.get_leaf(idx).unwrap().unwrap();
    });
    let use_time = SystemTime::now().duration_since(begin).unwrap();
    println!(
        "test accumulator get leaves, leaves count: {:?} use time: {:?} average time:{:?}",
        new_num_leaves,
        use_time,
        use_time.as_nanos() / new_num_leaves as u128,
    );
}

#[test]
fn test_accumulator_append() {
    // expected_root_hashes[i] is the root hash of an accumulator that has the first i leaves.
    let expected_root_hashes = (2000..2100).map(|x| {
        let leaves = create_leaves(2000..x);
        compute_root_hash_naive(&leaves)
    });

    let leaves = create_leaves(2000..2100);
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );

    // test to append empty leaf to an empty accumulator
    accumulator.append(&[]).unwrap();

    // Append the leaves one at a time and check the root hashes match.
    for (i, (leaf, expected_root_hash)) in
        itertools::zip_eq(leaves.into_iter(), expected_root_hashes).enumerate()
    {
        assert_eq!(accumulator.root_hash(), expected_root_hash);
        assert_eq!(accumulator.num_leaves(), i as LeafCount);
        accumulator.append(&[leaf]).unwrap();
    }

    // test to append empty leaf to an accumulator which isn't empty
    accumulator.append(&[]).unwrap();
}

#[test]
fn test_error_on_bad_parameters() {
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    assert!(accumulator.get_proof(10).unwrap().is_none());
}

#[test]
fn test_append_dup() {
    let mock_store = MockAccumulatorStore::new();
    let mock_store_arc = Arc::new(mock_store);
    let tx_accumulator = MerkleAccumulator::new_empty(mock_store_arc.clone());

    // proof verify
    let leaves_0 = create_leaves(0..10);
    let root_0 = tx_accumulator.append(&leaves_0).unwrap();
    proof_verify(&tx_accumulator, root_0, &leaves_0, 0);

    // append duplicate leaves and verify duplicate leaves
    tx_accumulator.append(&leaves_0[9..10]).unwrap();
    let root_1 = tx_accumulator.root_hash();
    assert_ne!(root_0, root_1);
    proof_verify(&tx_accumulator, root_1, &leaves_0[9..10], 10);

    // append new leaves and verify all
    let leaves_1 = create_leaves(10..20);
    let root_2 = tx_accumulator.append(&leaves_1).unwrap();
    proof_verify(&tx_accumulator, root_2, &leaves_0, 0);
    proof_verify(&tx_accumulator, root_2, &leaves_0[9..10], 10);
    proof_verify(&tx_accumulator, root_2, &leaves_1, 11);
}

#[test]
fn test_pop_unsaved() {
    let mock_store = MockAccumulatorStore::new();
    let mock_store_arc = Arc::new(mock_store);
    let tx_accumulator = MerkleAccumulator::new_empty(mock_store_arc.clone());
    let leaves = vec![H256::random(), H256::random(), H256::random()];
    let _root = tx_accumulator.append(&leaves).unwrap();
    let accumulator_info = tx_accumulator.get_info();

    let num_leaves = accumulator_info.num_leaves;
    let tx_accumulator_unsaved =
        MerkleAccumulator::new_with_info(accumulator_info.clone(), mock_store_arc.clone());
    for i in 0..num_leaves - 1 {
        let leaf = tx_accumulator_unsaved.get_leaf(i);
        assert!(leaf.is_err());
    }
    // the last leaf should be in frozen_subtree_roots, so it should be found.
    assert_eq!(
        leaves[num_leaves as usize - 1],
        tx_accumulator_unsaved
            .get_leaf(num_leaves - 1)
            .unwrap()
            .unwrap()
    );

    let unsaved_nodes = tx_accumulator.pop_unsaved_nodes();
    mock_store_arc.save_nodes(unsaved_nodes.unwrap()).unwrap();
    let tx_accumulator_saved =
        MerkleAccumulator::new_with_info(accumulator_info, mock_store_arc.clone());
    for i in 0..num_leaves {
        let leaf = tx_accumulator_saved.get_leaf(i);
        assert!(leaf.is_ok());
        assert_eq!(leaves[i as usize], leaf.unwrap().unwrap());
    }
}

#[test]
fn test_multiple_chain() {
    let leaves = create_leaves(50..52);
    let mock_store = Arc::new(MockAccumulatorStore::new());
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        mock_store.clone(),
    );
    let root_hash = accumulator.append(&leaves).unwrap();
    accumulator.flush().unwrap();
    proof_verify(&accumulator, root_hash, &leaves, 0);
    let frozen_node = accumulator.get_frozen_subtree_roots();
    for node in frozen_node.clone() {
        let acc = mock_store
            .get_node(node)
            .expect("get accumulator node by hash should success")
            .unwrap();
        if let AccumulatorNode::Internal(internal) = acc {
            let left = mock_store.get_node(internal.left()).unwrap().unwrap();
            assert!(left.is_frozen());
            let right = mock_store.get_node(internal.right()).unwrap().unwrap();
            assert!(right.is_frozen());
        }
    }
    let accumulator2 = MerkleAccumulator::new(root_hash, frozen_node, 2, 3, mock_store);
    assert_eq!(accumulator.root_hash(), accumulator2.root_hash());
    let leaves2 = create_leaves(54..58);
    let leaves3 = create_leaves(60..64);

    let _root_hash2 = accumulator.append(&leaves2).unwrap();
    accumulator.flush().unwrap();
    let _root_hash3 = accumulator2.append(&leaves3).unwrap();
    accumulator2.flush().unwrap();
    assert_eq!(
        accumulator.get_node_by_position(1).unwrap().unwrap(),
        accumulator2.get_node_by_position(1).unwrap().unwrap()
    );
    for i in 3..accumulator2.num_nodes() {
        assert_ne!(
            accumulator.get_node_by_position(i).unwrap().unwrap(),
            accumulator2.get_node_by_position(i).unwrap().unwrap()
        );
    }
}

#[test]
fn test_one_leaf() {
    let hash = H256::random();
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    assert_eq!(accumulator.get_index_frozen_subtrees().len(), 0);
    let root_hash = accumulator.append(&[hash]).unwrap();
    let frozen_subtrees = accumulator.get_index_frozen_subtrees();
    assert_eq!(frozen_subtrees.len(), 1);
    assert_eq!(
        frozen_subtrees.get(&NodeIndex::from_leaf_index(0u64)),
        Some(&hash)
    );

    assert_eq!(hash, root_hash);
    proof_verify(&accumulator, root_hash, &[hash], 0);
    let new_hash = H256::random();
    let new_root_hash = accumulator.append(&[new_hash]).unwrap();
    proof_verify(&accumulator, new_root_hash, &[new_hash], 1);
    let vec = vec![hash, new_hash];
    proof_verify(&accumulator, new_root_hash, &vec, 0);
}

#[test]
fn test_proof() {
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let batch1 = create_leaves(500..600);
    let root_hash1 = accumulator.append(&batch1).unwrap();
    accumulator.flush().unwrap();
    proof_verify(&accumulator, root_hash1, &batch1, 0);
}

#[test]
fn test_multiple_leaves() {
    let mut batch1 = create_leaves(600..608);
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let root_hash1 = accumulator.append(&batch1).unwrap();
    proof_verify(&accumulator, root_hash1, &batch1, 0);
    let batch2 = create_leaves(609..613);
    let root_hash2 = accumulator.append(&batch2).unwrap();
    batch1.extend_from_slice(&batch2);
    proof_verify(&accumulator, root_hash2, &batch1, 0);
}

#[test]
fn test_multiple_tree() {
    let batch1 = create_leaves(700..708);
    let mock_store = MockAccumulatorStore::new();
    let arc_store = Arc::new(mock_store);
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        arc_store.clone(),
    );
    let root_hash1 = accumulator.append(&batch1).unwrap();
    accumulator.flush().unwrap();
    proof_verify(&accumulator, root_hash1, &batch1, 0);
    let frozen_hash = accumulator.get_frozen_subtree_roots();
    let accumulator2 = MerkleAccumulator::new(root_hash1, frozen_hash, 8, 15, arc_store);
    let root_hash2 = accumulator2.root_hash();
    assert_eq!(root_hash1, root_hash2);
    proof_verify(&accumulator2, root_hash2, &batch1, 0);
}

#[test]
fn test_update_left_leaf() {
    // construct a accumulator
    let leaves = create_leaves(800..820);
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let root_hash = accumulator.append(&leaves).unwrap();
    proof_verify(&accumulator, root_hash, &leaves, 0);
}
#[test]
fn test_update_right_leaf() {
    // construct a accumulator
    let leaves = create_leaves(900..920);
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let root_hash = accumulator.append(&leaves).unwrap();
    proof_verify(&accumulator, root_hash, &leaves, 0);
}
#[test]
fn test_flush() {
    let leaves = create_leaves(1000..1020);
    let mock_store = Arc::new(MockAccumulatorStore::new());
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        mock_store.clone(),
    );
    let _root_hash = accumulator.append(&leaves).unwrap();
    accumulator.flush().unwrap();
    //get from storage
    for node_hash in leaves {
        let node = mock_store.get_node(node_hash).unwrap();
        assert!(node.is_some());
    }
}

#[test]
fn test_get_leaves_batch() {
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let leaves: Vec<H256> = (0..100).map(|_| H256::random()).collect();
    let _root_hash = accumulator.append(leaves.as_slice()).unwrap();
    accumulator.flush().unwrap();

    let leaves0 = accumulator.get_leaves(0, false, 100).unwrap();
    assert_eq!(leaves0.len(), 100);
    assert_eq!(leaves.as_slice(), leaves0.as_slice());

    let leaves1 = accumulator.get_leaves(5, true, 100).unwrap();
    assert_eq!(leaves1.len(), 6);
    assert_eq!(
        (0..6usize).rev().map(|i| leaves[i]).collect::<Vec<_>>(),
        leaves1
    );

    let leaves2 = accumulator.get_leaves(5, false, 90).unwrap();
    assert_eq!(leaves2.len(), 90);
    assert_eq!(&leaves[5..5 + 90], leaves2.as_slice());

    let leaves3 = accumulator.get_leaves(5, false, 100).unwrap();
    assert_eq!(leaves3.len(), 100 - 5);
    assert_eq!(&leaves[5..], leaves3.as_slice());
}

#[test]
fn test_get_leaves_overflow() {
    let mock_store = MockAccumulatorStore::new();
    let accumulator = MerkleAccumulator::new(
        *ACCUMULATOR_PLACEHOLDER_HASH,
        vec![],
        0,
        0,
        Arc::new(mock_store),
    );
    let leaves: Vec<H256> = (0..100).map(|_| H256::random()).collect();
    let _root_hash = accumulator.append(leaves.as_slice()).unwrap();
    accumulator.flush().unwrap();

    let leaves0 = accumulator.get_leaves(0, false, u64::MAX).unwrap();
    assert_eq!(leaves0.len(), 100);
    assert_eq!(leaves.as_slice(), leaves0.as_slice());

    let leaves1 = accumulator.get_leaves(u64::MAX, true, u64::MAX).unwrap();
    assert_eq!(leaves1.len(), 100);
}

#[test]
fn test_get_frozen_subtrees() {}

fn proof_verify(
    accumulator: &MerkleAccumulator,
    root_hash: H256,
    leaves: &[H256],
    first_leaf_idx: u64,
) {
    leaves.iter().enumerate().for_each(|(i, hash)| {
        let leaf_index = first_leaf_idx + i as u64;
        let proof = accumulator.get_proof(leaf_index).unwrap().unwrap();
        assert!(
            proof.verify(root_hash, *hash, leaf_index).is_ok(),
            "leaf_index:{}, proof:{:?} verify failed",
            leaf_index,
            proof
        );
    });
}

// Helper function to create a list of leaves.
fn create_leaves(nums: std::ops::Range<usize>) -> Vec<H256> {
    nums.map(|x| sha3_256_of(x.to_be_bytes().as_ref()))
        .collect()
}

// Computes the root hash of an accumulator with given leaves.
fn compute_root_hash_naive(leaves: &[H256]) -> H256 {
    let position_to_hash = compute_hashes_for_all_positions(leaves);
    if position_to_hash.is_empty() {
        return *ACCUMULATOR_PLACEHOLDER_HASH;
    }

    let rightmost_leaf_index = leaves.len() as u64 - 1;
    *position_to_hash
        .get(&NodeIndex::root_from_leaf_index(rightmost_leaf_index))
        .expect("Root position should exist in the map.")
}

/// Given a list of leaves, constructs the smallest accumulator that has all the leaves and
/// computes the hash of every node in the tree.
fn compute_hashes_for_all_positions(leaves: &[H256]) -> HashMap<NodeIndex, H256> {
    if leaves.is_empty() {
        return HashMap::new();
    }

    let mut current_leaves = leaves.to_vec();
    current_leaves.resize(
        leaves.len().next_power_of_two(),
        *ACCUMULATOR_PLACEHOLDER_HASH,
    );
    let mut position_to_hash = HashMap::new();
    let mut current_level = 0;

    while current_leaves.len() > 1 {
        assert!(current_leaves.len().is_power_of_two());

        let mut parent_leaves = vec![];
        for (index, _hash) in current_leaves.iter().enumerate().step_by(2) {
            let left_hash = current_leaves[index];
            let right_hash = current_leaves[index + 1];
            let left_pos = NodeIndex::from_level_and_pos(current_level, index as u64);
            let right_pos = NodeIndex::from_level_and_pos(current_level, index as u64 + 1);
            let parent_index = left_pos.parent();
            let parent_hash = compute_parent_hash(parent_index, left_hash, right_hash);
            parent_leaves.push(parent_hash);

            assert_eq!(position_to_hash.insert(left_pos, left_hash), None);
            assert_eq!(position_to_hash.insert(right_pos, right_hash), None);
        }

        assert_eq!(current_leaves.len(), parent_leaves.len() << 1);
        current_leaves = parent_leaves;
        current_level += 1;
    }

    assert_eq!(
        position_to_hash.insert(
            NodeIndex::from_level_and_pos(current_level, 0),
            current_leaves[0],
        ),
        None,
    );
    position_to_hash
}

fn compute_parent_hash(node_index: NodeIndex, left_hash: H256, right_hash: H256) -> H256 {
    if left_hash == *ACCUMULATOR_PLACEHOLDER_HASH && right_hash == *ACCUMULATOR_PLACEHOLDER_HASH {
        *ACCUMULATOR_PLACEHOLDER_HASH
    } else {
        AccumulatorNode::new_internal(node_index, left_hash, right_hash).hash()
    }
}
