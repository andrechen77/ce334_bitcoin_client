use serde::{Serialize, Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, ED25519, KeyPair, UnparsedPublicKey};
use crate::crypto::hash::{Hashable, H256};
use std::iter::FromIterator;
use rand::{prelude::*, distributions::Alphanumeric};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawTransaction {
    pub author: String, // the name of the person who said something
    pub statement: String, // the statement that they said
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
    public_key.verify(&transaction_bytes, signature_bytes).is_ok()
}

impl Hashable for RawTransaction {
    fn hash(&self) -> H256 {
        let bytes = bincode::serialize(&self).expect("shouldn't fail");
        ring::digest::digest(&ring::digest::SHA256, &bytes).into()
    }
}

pub struct SignedTransaction {
    pub transaction: RawTransaction,
    pub signature: Signature,
}

pub type Transaction = RawTransaction;

pub fn generate_random_transaction() -> RawTransaction {
    let mut rng = SmallRng::from_entropy();
    let author = String::from_iter(rng.sample_iter(&Alphanumeric).take(8));
    let statement = String::from_iter(rng.sample_iter(&Alphanumeric).take(32));
    RawTransaction { author, statement }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::crypto::key_pair;

    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
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
                let t = generate_random_transaction();
                let key = key_pair::random();
                let signature = sign(&t, &key);
                assert!(verify(&t, &(key.public_key()), &signature));
            }
        }
    }
}
