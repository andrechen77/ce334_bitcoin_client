use super::hash::{Hashable, H256};

#[derive(Debug, Default, Clone)]
struct MerkleTreeNode {
    hash: H256,
    lhs: Option<Box<MerkleTreeNode>>,
    rhs: Option<Box<MerkleTreeNode>>,
}

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    root: Box<MerkleTreeNode>,
    num_aggregations: usize, // one less than the number of levels
}

/// Given the hash of the left and right nodes, compute the hash of the parent node.
fn hash_children(lhs: &H256, rhs: &H256) -> H256 {
    let concatenation = [lhs.as_ref(), rhs.as_ref()].concat();
    ring::digest::digest(&ring::digest::SHA256, &concatenation).into()
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self
    where
        T: Hashable,
    {
        assert!(!data.is_empty());

        // turn each item into the leaf nodes
        let mut nodes: Vec<_> = data
            .iter()
            .map(|item| {
                Some(Box::new(MerkleTreeNode {
                    hash: item.hash(),
                    lhs: None,
                    rhs: None,
                }))
            })
            .collect();

        // aggregate the nodes together until there is only one root
        let mut num_aggregations = 0;
        let mut num_remaining_nodes = nodes.len();
        while num_remaining_nodes > 1 {
            // make one pass through the array, aggregating node pairs together
            'single_pass: for i in 0..=(nodes.len() / 2) {
                // helper function that takes the Box<MerkleTreeNode> at an
                // index that might be out of bounds
                fn get_node(
                    vec: &mut Vec<Option<Box<MerkleTreeNode>>>,
                    index: usize,
                ) -> Option<Box<MerkleTreeNode>> {
                    vec.get_mut(index).unwrap_or(&mut None).take()
                }

                // get the left node if it exists
                let Some(lhs) = get_node(&mut nodes, i * 2) else {
                    // we're out of nodes for this pass
                    break 'single_pass;
                };

                // take either the next node or clone the left node
                let rhs = get_node(&mut nodes, i * 2 + 1).unwrap_or_else(|| lhs.clone());

                // replace the left node's spot with a parent that has both
                // lhs and rhs as children
                nodes[i] = Some(Box::new(MerkleTreeNode {
                    hash: hash_children(&lhs.hash, &rhs.hash),
                    lhs: Some(lhs),
                    rhs: Some(rhs),
                }));
            }

            num_remaining_nodes = (num_remaining_nodes + 1) / 2;
            num_aggregations += 1;
        }

        assert!(num_remaining_nodes == 1);
        MerkleTree {
            root: nodes[0].take().expect("there remains exactly one node"),
            num_aggregations,
        }
    }

    pub fn root(&self) -> H256 {
        self.root.hash
    }

    /// Returns the Merkle Proof of data at index i. The closest sibling hash
    /// is at the end of the returned array; i.e. the proof goes top-down
    pub fn proof(&self, index: usize) -> Vec<H256> {
        // the binary representation of the index, read from MSB to LSB, serves
        // as sequence of instructions to locate the `index`th leaf, where 0
        // means to go left (0th child) and 1 means to go right (1th child)

        // find the directions by decomposing the index
        let mut bit_path = index;
        let mut directions = Vec::new(); // store the
        for _ in 0..self.num_aggregations {
            directions.push(bit_path & 1 == 1); // query the LSB
            bit_path >>= 1;
        }

        let mut result = Vec::new();
        let mut current_node = &self.root;
        for direction in directions {
            const ERROR_MSG: &str =
                "can traverse through `self.num_aggregations` levels; the tree is full";
            if direction {
                // go right
                result.push(current_node.lhs.as_ref().expect(ERROR_MSG).hash);
                current_node = current_node.rhs.as_ref().expect(ERROR_MSG);
            } else {
                // go left
                result.push(current_node.rhs.as_ref().expect(ERROR_MSG).hash);
                current_node = current_node.lhs.as_ref().expect(ERROR_MSG);
            };
        }
        result
    }
}

/// Verify that the datum hash with a vector of proofs will produce the Merkle root. Also need the
/// index of datum and `leaf_size`, the total number of leaves.
pub fn verify(
    root_hash: &H256,
    datum_hash: &H256,
    proof: &[H256],
    index: usize,
    _num_leaves: usize,
) -> bool {
    let mut bit_path = index;
    let mut current_hash = *datum_hash;
    for sibling_hash in proof.iter().rev() {
        let direction = bit_path & 1 == 1; // true iff the current node is a right child
        bit_path >>= 1;

        if direction {
            // current node is a right child
            current_hash = hash_children(sibling_hash, &current_hash);
        } else {
            // current node is a left child
            current_hash = hash_children(&current_hash, sibling_hash);
        }
    }
    *root_hash == current_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hash::H256;

    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
            ]
        }};
    }

    macro_rules! gen_merkle_tree_large {
        () => {{
            vec![
                (hex!("0000000000000000000000000000000000000000000000000000000000000011")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000022")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000033")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000044")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000055")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000066")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000077")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000088")).into(),
            ]
        }};
    }

    #[test]
    fn root() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }

    #[test]
    fn proof() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(
            proof,
            vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into()]
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }

    #[test]
    fn proof_tree_large() {
        let input_data: Vec<H256> = gen_merkle_tree_large!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(5);

        // We accept the proof in either the top-down or bottom-up order; you should stick to either of them.
        let expected_proof_bottom_up: Vec<H256> = vec![
            (hex!("c8c37c89fcc6ee7f5e8237d2b7ed8c17640c154f8d7751c774719b2b82040c76")).into(),
            (hex!("bada70a695501195fb5ad950a5a41c02c0f9c449a918937267710a0425151b77")).into(),
            (hex!("1e28fb71415f259bd4b0b3b98d67a1240b4f3bed5923aa222c5fdbd97c8fb002")).into(),
        ];
        let expected_proof_top_down: Vec<H256> = vec![
            (hex!("1e28fb71415f259bd4b0b3b98d67a1240b4f3bed5923aa222c5fdbd97c8fb002")).into(),
            (hex!("bada70a695501195fb5ad950a5a41c02c0f9c449a918937267710a0425151b77")).into(),
            (hex!("c8c37c89fcc6ee7f5e8237d2b7ed8c17640c154f8d7751c774719b2b82040c76")).into(),
        ];
        assert!(proof == expected_proof_bottom_up || proof == expected_proof_top_down);
    }

    #[test]
    fn verifying() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(
            &merkle_tree.root(),
            &input_data[0].hash(),
            &proof,
            0,
            input_data.len()
        ));
    }

    #[cfg(feature = "my-tests")]
    mod my_tests {
        use super::*;

        #[test]
        fn non_power_of_two_root() {
            let input_data: Vec<H256> = vec![
                (hex!("0000000000000000000000000000000000000000000000000000000000000011")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000022")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000033")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000044")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000055")).into(),
                (hex!("0000000000000000000000000000000000000000000000000000000000000066")).into(),
            ];
            let merkle_tree = MerkleTree::new(&input_data);
            let root = merkle_tree.root();
            assert_eq!(
                root,
                (hex!("fec4ab32f934781325d07c3fbcb48d2bbd354ae0b699ac166b9e7774010067aa")).into()
            );
        }
    }
}
