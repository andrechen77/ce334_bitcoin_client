use crate::transaction::SignedTransaction as Transaction;
use std::collections::{hash_map::Entry, HashMap};
use crate::crypto::hash::{H256, Hashable};

/// Store all the received valid transactions which have not been included in the blockchain yet.
pub struct Mempool {
    hash_to_transaction: HashMap<H256, Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Mempool {
            hash_to_transaction: HashMap::new(),
        }
    }

    /// Get a transaction from the mempool by hash (or `None` if it does not exist)
    pub fn get_transaction(&self, hash: &H256) -> Option<&Transaction> {
        self.hash_to_transaction.get(hash)
    }

    /// Insert a transaction into the mempool
    pub fn insert(&mut self, transaction: Transaction) {
        // (Make sure you have implemented the `Hashable` trait for `SignedTransaction`, or there will be an error):
        let hash = transaction.hash();
        self.hash_to_transaction.insert(hash, transaction);
    }

    /// Insert a transaction into the mempool with validation. Returns whether
    /// the transaction was added.
    pub fn insert_with_validation(&mut self, transaction: Transaction) -> bool {
        let hash = transaction.hash();

        match self.hash_to_transaction.entry(hash) {
            Entry::Occupied(_) => {
                // the transaction is already in the mempool
                false
            }
            Entry::Vacant(entry) => {
                // validate the transaction
                // check its signature
                if !transaction.verify_signature() {
                    return false;
                }
                // TODO check for double spending
                unimplemented!();
                // add the transaction to the mempool
                entry.insert(transaction);
            }
        }
    }

    /// Remove a random transaction from the mempool and return it (or `None` if it is empty)
    pub fn pop(&mut self) -> Option<Transaction> {
        let hash = self.hash_to_transaction.keys().next().cloned();
        if let Some(hash) = hash {
            self.hash_to_transaction.remove(&hash)
        } else {
            None
        }
    }

    // TODO Optional: you may want to add more methods here...
}