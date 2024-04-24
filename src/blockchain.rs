use crate::block::Block;
use crate::crypto::hash::{Hashable, H256};
use std::collections::HashMap;

pub struct Blockchain {
    hash_to_block: HashMap<H256, (Block, u64)>,
    tip: H256,
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let genesis = Block::genesis();
        let genesis_hash = genesis.hash();
        Blockchain {
            hash_to_block: HashMap::from([(genesis_hash, (genesis, 0))]),
            tip: genesis_hash,
        }
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        let hash = block.hash();
        let &(_, parent_height) = self
            .hash_to_block
            .get(&block.header.parent)
            .expect("no orphan blocks");
        let block_height = parent_height + 1;
        self.hash_to_block
            .insert(hash, (block.clone(), block_height));

        // if the block's height is the new tallest, it becomes the new tip
        let &(_, current_tallest_height) = self
            .hash_to_block
            .get(&self.tip)
            .expect("tip exists in the blockchain");
        if block_height > current_tallest_height {
            self.tip = hash;
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.tip
    }

    /// Get all the blocks' hashes along the longest chain
    #[cfg(any(test, test_utilities))]
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut results = Vec::new();
        let mut current_hash = self.tip;

        let &(_, expected_height) = self
            .hash_to_block
            .get(&self.tip)
            .expect("tip exists in the blockchain");

        while let Some((block, _)) = self.hash_to_block.get(&current_hash) {
            results.push(current_hash);
            current_hash = block.header.parent;
        }

        assert_eq!(results.len() as u64, expected_height + 1);

        results
    }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::block::test::generate_random_block;
    use crate::crypto::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());
    }

    #[test]
    fn mp1_insert_chain() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let mut block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());
        for _ in 0..50 {
            let h = block.hash();
            block = generate_random_block(&h);
            blockchain.insert(&block);
            assert_eq!(blockchain.tip(), block.hash());
        }
    }

    #[test]
    fn mp1_insert_3_fork_and_back() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block_1 = generate_random_block(&genesis_hash);
        blockchain.insert(&block_1);
        assert_eq!(blockchain.tip(), block_1.hash());
        let block_2 = generate_random_block(&block_1.hash());
        blockchain.insert(&block_2);
        assert_eq!(blockchain.tip(), block_2.hash());
        let block_3 = generate_random_block(&block_2.hash());
        blockchain.insert(&block_3);
        assert_eq!(blockchain.tip(), block_3.hash());
        let fork_block_1 = generate_random_block(&block_2.hash());
        blockchain.insert(&fork_block_1);
        assert_eq!(blockchain.tip(), block_3.hash());
        let fork_block_2 = generate_random_block(&fork_block_1.hash());
        blockchain.insert(&fork_block_2);
        assert_eq!(blockchain.tip(), fork_block_2.hash());
        let block_4 = generate_random_block(&block_3.hash());
        blockchain.insert(&block_4);
        assert_eq!(blockchain.tip(), fork_block_2.hash());
        let block_5 = generate_random_block(&block_4.hash());
        blockchain.insert(&block_5);
        assert_eq!(blockchain.tip(), block_5.hash());
    }

    #[cfg(feature = "my-tests")]
    mod my_tests {
        use super::*;

        #[test]
        fn hash_chain() {
            let mut blockchain = Blockchain::new();
            let genesis_hash = blockchain.tip();
            let block_1 = generate_random_block(&genesis_hash);
            blockchain.insert(&block_1);
            let block_2 = generate_random_block(&block_1.hash());
            blockchain.insert(&block_2);
            let block_3 = generate_random_block(&block_2.hash());
            blockchain.insert(&block_3);
            let block_4 = generate_random_block(&block_3.hash());
            blockchain.insert(&block_4);
            let block_5 = generate_random_block(&block_4.hash());
            blockchain.insert(&block_5);
            let blocks_in_longest_chain = blockchain.all_blocks_in_longest_chain();
            assert_eq!(
                blocks_in_longest_chain,
                vec![
                    block_5.hash(),
                    block_4.hash(),
                    block_3.hash(),
                    block_2.hash(),
                    block_1.hash(),
                    genesis_hash,
                ],
            );
        }
    }
}
