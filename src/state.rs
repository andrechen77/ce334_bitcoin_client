use core::fmt;
use std::collections::HashMap;
use log::{debug, warn};
use ring::signature::{Ed25519KeyPair, KeyPair};
use crate::{crypto::{address::H160, key_pair::get_deterministic_keypair}, transaction::RawTransaction};

#[derive(Clone, Debug)]
pub struct AccountInfo {
    /// represents the nonce of the next valid transaction
	pub nonce: u32,
	pub balance: u64,
}

impl AccountInfo {
    pub fn new() -> Self {
        AccountInfo {
            nonce: 0,
            balance: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct State {
	pub_key_to_acc_info: HashMap<H160, AccountInfo>,
}

impl State {
    /// Initial coin offering; generate an initial state.
    pub fn ico() -> Self {
        let mut pub_key_to_acc_info = HashMap::new();
        // give the i-th account 1000 * (10 - i) coins, i = 0, 1, 2, ..., 9
        for i in 0..10 {
            let pair = get_deterministic_keypair(i);
            let address = H160::from_pubkey(pair.public_key().as_ref());
            let balance: u64 = 1000 * ((10 - i) as u64);
            let nonce: u32 = 0;
            pub_key_to_acc_info.insert(address, AccountInfo { nonce, balance });
        }
        State { pub_key_to_acc_info }
    }

    pub fn check_transaction_validity(&self, transaction: &RawTransaction) -> bool {
        let RawTransaction { from_addr, to_addr: _, nonce, value } = transaction;

        let Some(spender_info) = self.pub_key_to_acc_info.get(from_addr) else {
            // if account doesn't exist, it has no money to spend
            return false;
        };
        if spender_info.nonce != *nonce {
            return false;
        }
        if spender_info.balance < *value {
            return false;
        }
        true
    }

    #[must_use]
	pub fn update_in_place(&mut self, transaction: &RawTransaction) -> bool {
        let RawTransaction { from_addr, to_addr, nonce, value } = transaction;

        // check for double spending

        let Some(spender_info) = self.pub_key_to_acc_info.get_mut(from_addr) else {
            // if account doesn't exist, it has no money to spend
            return false;
        };

        if spender_info.nonce != *nonce {
            return false;
        }
        if spender_info.balance < *value {
            return false;
        }

        // the transaction is valid, go through with it
        spender_info.nonce += 1;
        spender_info.balance -= value;
        let receiver_info = self
            .pub_key_to_acc_info
            .entry(to_addr.clone())
            .or_insert_with(AccountInfo::new);
        receiver_info.balance += value;
        true
	}

    /// Returns a new State representing what would happen if the given
    /// transactions acted on this State. Returns None if the transactions
    /// are invalid.
    pub fn update_with_transactions<'a>(
        &self,
        transactions: impl Iterator<Item = &'a RawTransaction>,
    ) -> Option<Self> {
        let mut updated = self.clone();
        let mut transactions = transactions;
        if transactions.all(|transaction| updated.update_in_place(transaction)) {
            Some(updated)
        } else {
            None
        }
    }

    pub fn get_acc_info(&self, addr: &H160) -> Option<&AccountInfo> {
        self.pub_key_to_acc_info.get(addr)
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ledger: Vec<_> = self.pub_key_to_acc_info.iter().collect();
        ledger.sort_by(|a, b| a.0.cmp(b.0));
        let ledger: Vec<_> = ledger.into_iter().map(|(hash, acc_info)| format!("{hash}: balance {}, nonce {}", acc_info.balance, acc_info.nonce)).collect();
        write!(f, "{:#?}", ledger)
    }
}
