use log::debug;
use serde::{Serialize,Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm, EdDSAParameters};
use crate::crypto;
use crate::crypto::address::H160;
use crate::crypto::hash::{H256, Hashable};

use crate::crypto::key_pair::get_deterministic_keypair;
use crate::network::server::Handle as ServerHandle;
use crate::transaction::{RawTransaction, SignedTransaction};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time;
use std::sync::{Arc, Mutex};
use crate::network::message::Message;
use crate::blockchain::{Blockchain};

pub struct TransactionGenerator {
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    rx: Receiver<()>,
}

impl TransactionGenerator {
    pub fn new(
        server: &ServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        rx: Receiver<()>,
    ) -> TransactionGenerator {
        TransactionGenerator {
            server: server.clone(),
            blockchain: Arc::clone(blockchain),
            rx,
        }
    }

    pub fn start(self) {
        thread::spawn(move || {
            if self.rx.recv().is_ok() {
                self.generation_loop();
            }
            log::warn!("Transaction Generator exited");
        });
    }

    /// Generate random transactions and send them to the server
    fn generation_loop(&self) {
        const INTERVAL_MILLISECONDS: u64 = 700; // how quickly to generate transactions

        let mut next_sender_acc = 0;
        loop {

            // sleep for some time:
            let _ = self.rx.recv();
            // let interval = time::Duration::from_millis(INTERVAL_MILLISECONDS);
            // thread::sleep(interval);

            let mut blockchain = self.blockchain.lock().expect("idk why this should work");

            // 1. generate some random transactions:
            let num_transactions = 1;
            let transactions: Vec<_> = std::iter::from_fn(|| {
                let receiver_acc_num = rand::random::<u8>() % 10;
                let sender_key_pair = get_deterministic_keypair(next_sender_acc);
                let receiver_key_pair = get_deterministic_keypair(receiver_acc_num);
                let from_addr = H160::from_pubkey(sender_key_pair.public_key().as_ref());
                let to_addr = H160::from_pubkey(receiver_key_pair.public_key().as_ref());
                let (_, _, latest_state) = blockchain.tip_data();
                let nonce = latest_state
                    .get_acc_info(&from_addr)
                    .expect("this account should have been in the ICO")
                    .nonce;
                let valid = rand::random::<u8>() % 8 != 0;
                Some(SignedTransaction::from_raw(
                    RawTransaction {
                        from_addr,
                        to_addr,
                        value: 1,
                        nonce,
                    },
                    if valid { &sender_key_pair } else { &receiver_key_pair },
                ))
            }).take(num_transactions).collect();

            debug!("generated transactions {:?}", transactions);

            // 2. add these transactions to the mempool:
            for transaction in &transactions {
                let _ = blockchain.insert_transaction_with_validation(transaction.clone());
            }
            // 3. broadcast them using `self.server.broadcast(Message::NewTransactionHashes(...))`:
            self.server.broadcast(Message::NewTransactionHashes(transactions.into_iter().map(|tx| tx.hash()).collect()));

            next_sender_acc += 1;
            next_sender_acc %= 10;
        }
    }
}
