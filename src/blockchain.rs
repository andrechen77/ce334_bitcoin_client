use log::{info, warn};

use crate::block::Block;
use crate::crypto::hash::{Hashable, H256};
use crate::state::State;
use crate::transaction::SignedTransaction;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Blockchain {
    /// Stores all the blocks in the chain. Maps the block's hash to its data.
    hash_to_block: HashMap<H256, (Block, u64, Arc<State>)>,
    /// Stores the hash of the block at the tip.
    tip: H256,
    /// Stores all the blocks whose parents we don't know about yet Maps the
    /// block's parent's hash to all the orphans depending on that parent
    orphanage: HashMap<H256, Vec<Block>>,
    /// Store all the received valid transactions which have not been included
    /// in the blockchain yet. Maps a transaction's hash to its data
    mempool: HashMap<H256, SignedTransaction>,
    /// Whether the mempool might have some invalid transactions due to state
    /// changes
    dirty_mempool: bool,
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let genesis = Block::genesis();
        let genesis_hash = genesis.hash();
        let initial_state = Arc::new(State::ico());
        Blockchain {
            hash_to_block: HashMap::from([(genesis_hash, (genesis, 0, initial_state))]),
            tip: genesis_hash,
            orphanage: HashMap::new(),
            mempool: HashMap::new(),
            dirty_mempool: false,
        }
    }

    /// Insert a block into blockchain
    /// should only be used for debugging
    pub fn insert_block(&mut self, block: Block) {
        let hash = block.hash();
        let (_, parent_height, parent_state) = self
            .hash_to_block
            .get(&block.header.parent)
            .expect("no orphan blocks");
        let block_height = *parent_height + 1;
        let new_state = parent_state.clone();
        self.hash_to_block.insert(hash, (block, block_height, new_state));

        // if the block's height is the new tallest, it becomes the new tip
        let &(_, current_tallest_height, _) = self
            .hash_to_block
            .get(&self.tip)
            .expect("tip exists in the blockchain");
        if block_height > current_tallest_height {
            self.tip = hash;
        }
    }

    /// Insert a block into the blockchain with validation. May assign orphan
    /// blocks to their parents. Returns all blocks that were added
    pub fn insert_block_with_validation(&mut self, block: Block) -> Vec<H256> {
        let mut added_blocks = vec![];

        // check if the block is already in the blockchain
        if self.hash_to_block.contains_key(&block.hash()) {
            return added_blocks;
        }

        // find the the parent
        let hash = block.hash();
        let parent_hash = &block.header.parent;
        if let Some((parent_block, parent_height, parent_state)) = self.hash_to_block.get(parent_hash) {
            // calculate the difficulty
            let required_difficulty = parent_block.header.difficulty;

            // validate the block
            // check its nonce
            if hash > required_difficulty {
                // reject the block
                return added_blocks;
            }
            // check all transactions inside it
            let Some(new_state) = parent_state.update_with_transactions(
                block.content.transactions.iter().map(|signed| &signed.raw_transaction)
            ) else {
                return added_blocks;
            };

            // block seems valid. assume that if the blocks are valid, then we
            // care about them even if they're unsolicited.

            // update the mempool
            // remove transactions that are in this block
            for transaction in &block.content.transactions {
                self.mempool.remove(&transaction.hash());
            }

            // add the block to the blockchain
            let block_height = parent_height + 1;
            info!("inserted block {}", hash);
            self.hash_to_block.insert(hash, (block, block_height, Arc::new(new_state)));

            // if the block's height is the new tallest, it becomes the new tip
            let &(_, current_tallest_height, _) = self
                .hash_to_block
                .get(&self.tip)
                .expect("tip exists in the blockchain");
            if block_height > current_tallest_height {
                self.tip = hash;
                self.dirty_mempool = true;
            }

            added_blocks.push(hash);

            // insert all blocks for which this block is a parent
            if let Some(orphan_children) = self.orphanage.remove(&hash) {
                for orphan in orphan_children {
                    let mut added_children = self.insert_block_with_validation(orphan);
                    added_blocks.append(&mut added_children);
                }
            }

            if self.dirty_mempool {
                self.prune_invalid_transactions();
            }
        } else {
            // put it into the orphanage
            self.orphanage.entry(*parent_hash).or_default().push(block);
        }
        added_blocks
    }

    /// Get the last block's hash of the longest chain
    pub fn tip_hash(&self) -> H256 {
        self.tip
    }

    /// Get the data of the tip
    pub fn tip_data(&self) -> (&Block, u64, &State) {
        let (block, height, state) = self.hash_to_block.get(&self.tip).expect("tip should exist");
        (block, *height, state)
    }

    /// Look up a block and its height and state using the specified hash
    pub fn look_up_block(&self, hash: &H256) -> Option<&(Block, u64, Arc<State>)> {
        self.hash_to_block.get(hash)
    }

    /// Get all the blocks' hashes along the longest chain
    #[cfg(any(test, test_utilities))]
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut results = Vec::new();
        let mut current_hash = self.tip;

        let &(_, expected_height, _) = self
            .hash_to_block
            .get(&self.tip)
            .expect("tip exists in the blockchain");

        while let Some((block, _, _)) = self.hash_to_block.get(&current_hash) {
            results.push(current_hash);
            current_hash = block.header.parent;
        }

        assert_eq!(results.len() as u64, expected_height + 1);

        results
    }

    /// Get a transaction from the mempool by hash (or `None` if it does not exist)
    pub fn get_transaction(&self, hash: &H256) -> Option<&SignedTransaction> {
        // TODO shouldn't this also check the entire blockchain ughh
        self.mempool.get(hash)
    }

    pub fn mempool_transactions(&self) -> impl Iterator<Item = (&H256, &SignedTransaction)> {
        self.mempool.iter()
    }

    #[must_use]
    pub fn insert_transaction_with_validation(&mut self, transaction: SignedTransaction) -> bool {
        let hash = transaction.hash();
        if self.get_transaction(&hash).is_some() {
            // the transaction is already in the mempool
            return false;
        }

        // validate the transaction
        // check its signature
        if !transaction.verify_signature() {
            info!("rejected transaction {:?}", transaction);
            return false;
        }
        let (_block, _height, state) = self.tip_data();
        if !state.check_transaction_validity(&transaction.raw_transaction) {
            return false;
        }

        // insert the transaction
        info!("inserted transaction {:?}", transaction);
        self.mempool.insert(hash, transaction);
        true
    }

    /// Removes all transactions from the mempool that might be invalid due
    /// to state changes
    fn prune_invalid_transactions(&mut self) {
        let (_, _, latest_state) = self.tip_data();
        let latest_state = latest_state.clone(); // TODO this is just to avoid memory issues, actually fix later
        self.mempool.retain(|_, transaction| {
            latest_state.check_transaction_validity(&transaction.raw_transaction)
        });
        self.dirty_mempool = false;
    }
}

impl std::fmt::Display for Blockchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (_, height, _) = self.tip_data();
        write!(
            f,
            "Blockchain status\nNum Blocks: {}\nTip: height {}, hash {}\nMempool: {:#?}\nLedger: {}",
            self.hash_to_block.len(),
            height,
            self.tip,
            self.mempool,
            self.tip_data().2,
        )
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
        let genesis_hash = blockchain.tip_hash();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert_block(block.clone());
        assert_eq!(blockchain.tip_hash(), block.hash());
    }

    #[test]
    fn mp1_insert_chain() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip_hash();
        let mut block = generate_random_block(&genesis_hash);
        blockchain.insert_block(block.clone());
        assert_eq!(blockchain.tip_hash(), block.hash());
        for _ in 0..50 {
            let h = block.hash();
            block = generate_random_block(&h);
            blockchain.insert_block(block.clone());
            assert_eq!(blockchain.tip_hash(), block.hash());
        }
    }

    #[test]
    fn mp1_insert_3_fork_and_back() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip_hash();
        let block_1 = generate_random_block(&genesis_hash);
        blockchain.insert_block(block_1.clone());
        assert_eq!(blockchain.tip_hash(), block_1.hash());
        let block_2 = generate_random_block(&block_1.hash());
        blockchain.insert_block(block_2.clone());
        assert_eq!(blockchain.tip_hash(), block_2.hash());
        let block_3 = generate_random_block(&block_2.hash());
        blockchain.insert_block(block_3.clone());
        assert_eq!(blockchain.tip_hash(), block_3.hash());
        let fork_block_1 = generate_random_block(&block_2.hash());
        blockchain.insert_block(fork_block_1.clone());
        assert_eq!(blockchain.tip_hash(), block_3.hash());
        let fork_block_2 = generate_random_block(&fork_block_1.hash());
        blockchain.insert_block(fork_block_2.clone());
        assert_eq!(blockchain.tip_hash(), fork_block_2.hash());
        let block_4 = generate_random_block(&block_3.hash());
        blockchain.insert_block(block_4.clone());
        assert_eq!(blockchain.tip_hash(), fork_block_2.hash());
        let block_5 = generate_random_block(&block_4.hash());
        blockchain.insert_block(block_5.clone());
        assert_eq!(blockchain.tip_hash(), block_5.hash());
    }

    #[cfg(feature = "my-tests")]
    mod my_tests {
        use super::*;

        #[test]
        fn hash_chain() {
            let mut blockchain = Blockchain::new();
            let genesis_hash = blockchain.tip_hash();
            let block_1 = generate_random_block(&genesis_hash);
            blockchain.insert_block(block_1.clone());
            let block_2 = generate_random_block(&block_1.hash());
            blockchain.insert_block(block_2.clone());
            let block_3 = generate_random_block(&block_2.hash());
            blockchain.insert_block(block_3.clone());
            let block_4 = generate_random_block(&block_3.hash());
            blockchain.insert_block(block_4.clone());
            let block_5 = generate_random_block(&block_4.hash());
            blockchain.insert_block(block_5.clone());
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
