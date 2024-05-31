use crate::crypto::{hash::{Hashable, H256}, address::H160, key_pair};
use rand::{distributions::Standard, prelude::*};
use ring::signature::{Ed25519KeyPair, KeyPair, Signature, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawTransaction {
    pub from_addr: H160,
    pub to_addr: H160,
    pub value: u64,
    pub nonce: u32,
}

/// Create digital signature of a transaction
pub fn sign(transaction: &RawTransaction, key: &Ed25519KeyPair) -> Signature {
    let transaction_bytes = bincode::serialize(transaction).expect("shouldn't fail");
    key.sign(&transaction_bytes)
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(
    transaction: &RawTransaction,
    public_key: &<Ed25519KeyPair as KeyPair>::PublicKey,
    signature: &Signature,
) -> bool {
    let public_key_bytes: &[u8] = public_key.as_ref();
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    let signature_bytes: &[u8] = signature.as_ref();
    let transaction_bytes = bincode::serialize(transaction).expect("shouldn't fail");
    public_key
        .verify(&transaction_bytes, signature_bytes)
        .is_ok()
}

impl RawTransaction {
    pub fn generate_random() -> Self {
        let mut rng = SmallRng::from_entropy();
        let from_addr: [u8; 20] = rng.sample_iter(&Standard).take(20).collect::<Vec<u8>>().try_into().unwrap();
        let to_addr: [u8; 20] = rng.sample_iter(&Standard).take(20).collect::<Vec<u8>>().try_into().unwrap();
        let value = rng.gen();
        let nonce = rng.gen();
        RawTransaction {
            from_addr: from_addr.into(),
            to_addr: to_addr.into(),
            value,
            nonce,
        }
    }
}

impl Hashable for RawTransaction {
    fn hash(&self) -> H256 {
        let bytes = bincode::serialize(&self).expect("shouldn't fail");
        ring::digest::digest(&ring::digest::SHA256, &bytes).into()
    }
}

/// A signed transaction
#[derive(Serialize, Deserialize, Clone)]
pub struct SignedTransaction {
    pub raw_transaction: RawTransaction,
    pub pub_key: Vec<u8>,
    pub signature: Vec<u8>,
}

impl SignedTransaction {
    /// Create a new transaction from a raw transaction and a key pair
    pub fn from_raw(raw_transaction: RawTransaction, key: &Ed25519KeyPair) -> SignedTransaction {
        let pub_key = key.public_key().as_ref().to_vec();
        let signature = sign(&raw_transaction, key).as_ref().to_vec();
        SignedTransaction { raw_transaction, pub_key, signature }
    }

    pub fn generate_random() -> Self {
        let raw_transaction = RawTransaction::generate_random();
        let key = key_pair::random();
        SignedTransaction::from_raw(raw_transaction, &key)
    }

    /// Verify the signature of this transaction
    pub fn verify_signature(&self) -> bool {
        let serialized_raw = bincode::serialize(&self.raw_transaction).unwrap();
        let public_key = ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519, &self.pub_key[..]
        );

        let valid_signature = public_key.verify(&serialized_raw, self.signature.as_ref()).is_ok();
        let signed_by_owner = H160::from_pubkey(&self.pub_key[..]) == self.raw_transaction.from_addr;
        valid_signature && signed_by_owner
    }
}

impl std::fmt::Debug for SignedTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.raw_transaction)
    }
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        let bytes = bincode::serialize(&self).expect("shouldn't fail");
        ring::digest::digest(&ring::digest::SHA256, &bytes).into()
    }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::crypto::key_pair;

    #[test]
    fn sign_verify() {
        let t = RawTransaction::generate_random();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, &(key.public_key()), &signature));
    }

    #[cfg(feature = "my-tests")]
    mod my_tests {
        use super::*;

        #[test]
        fn sign_verify() {
            for _ in 0..100 {
                let t = RawTransaction::generate_random();
                let key = key_pair::random();
                let signature = sign(&t, &key);
                assert!(verify(&t, &(key.public_key()), &signature));
            }
        }
    }
}
