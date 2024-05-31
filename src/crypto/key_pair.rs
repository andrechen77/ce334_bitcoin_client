use ring::rand;
use ring::signature::Ed25519KeyPair;

/// Generate a random key pair.
pub fn random() -> Ed25519KeyPair {
    let rng = rand::SystemRandom::new();
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref().into()).unwrap()
}

// for Initial coin offering:
/// Get a deterministic keypair from a nonce:
pub fn get_deterministic_keypair(nonce: u8) -> Ed25519KeyPair {
    let mut seed = [0u8; 32];
    seed[0] = nonce;
    let keypair = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
    keypair
}
