use std::collections::HashMap;
use ring::signature::{Ed25519KeyPair, KeyPair};
use crate::{crypto::address::H160, transaction::RawTransaction};

#[derive(Clone)]
pub struct AccountInfo {
	nonce: u32,
	balance: u64,
}

#[derive(Clone)]
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

	/// Returns a new State representing what would happen if the given
	/// transactions acted on this State. Returns None if the transactions
	/// are invalid.
	pub fn update_with_transactions<'a>(
		&self,
		transactions: impl Iterator<Item = &'a RawTransaction>,
	) -> Option<Self> {
		// TODO complete this
		Some(self.clone())
	}
}

// for Initial coin offering:
/// Get a deterministic keypair from a nonce:
pub fn get_deterministic_keypair(nonce: u8) -> Ed25519KeyPair {
    let mut seed = [0u8; 32];
    seed[0] = nonce;
    let keypair = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
    keypair
}
