use crate::{
    crypto::hash::{Hashable, H256},
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};

/// the block header
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub parent: H256,
    pub nonce: u32,
    pub difficulty: H256, // lower is harder
    pub timestamp: u128,
    pub merkle_root: H256,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Content {
    pub transactions: Vec<Transaction>, // TODO consider using SignedTransaction
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: Header,
    pub content: Content,
}

// Returns the default difficulty, which is a big-endian 32-byte integer.
// For a valid block, block.hash() <= difficulty
fn default_difficulty() -> H256 {
    [0; 32].into()
}

impl Block {
    // deterministically construct the genesis block
    pub fn genesis() -> Block {
        Block {
            header: Header {
                parent: Default::default(),
                nonce: 0, // TODO is this supposed to be correct?
                difficulty: default_difficulty(),
                timestamp: 0,
                merkle_root: Default::default(),
            },
            content: Content {
                transactions: Vec::new(),
            },
        }
    }
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        let bytes = bincode::serialize(&self).expect("shouldn't fail");
        ring::digest::digest(&ring::digest::SHA256, &bytes).into()
    }
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        self.header.hash()
    }
}

#[cfg(any(test, test_utilities))]
pub mod test {
    use super::*;
    use crate::{
        crypto::{hash::H256, merkle::MerkleTree},
        transaction::generate_random_transaction,
    };

    pub fn generate_random_block(parent: &H256) -> Block {
        let transactions: Vec<Transaction> = vec![generate_random_transaction()];
        let root = MerkleTree::new(&transactions).root();
        Block {
            header: Header {
                parent: *parent,
                nonce: rand::random(),
                difficulty: default_difficulty(),
                timestamp: rand::random(),
                merkle_root: root,
            },
            content: Content { transactions },
        }
    }
}
