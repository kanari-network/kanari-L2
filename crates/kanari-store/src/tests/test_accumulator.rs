// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::KanariStore;
use accumulator::node_index::NodeIndex;
use accumulator::{AccumulatorNode, AccumulatorTreeStore};
use moveos_types::h256::H256;

#[tokio::test]
async fn test_accumulator_store() {
    let (kanari_store, _) = KanariStore::mock_kanari_store().unwrap();

    let acc_node = AccumulatorNode::new_leaf(NodeIndex::from_inorder_index(1), H256::random());
    let node_hash = acc_node.hash();
    kanari_store
        .transaction_accumulator_store
        .save_node(acc_node.clone())
        .unwrap();
    let acc_node2 = kanari_store
        .transaction_accumulator_store
        .get_node(node_hash)
        .unwrap()
        .unwrap();
    assert_eq!(acc_node, acc_node2);
}
