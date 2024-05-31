use super::message::Message;
use super::peer;
use crate::{
    block::Block,
    blockchain::Blockchain,
    crypto::hash::{Hashable, H256},
    network::server::Handle as ServerHandle,
    transaction::SignedTransaction as Transaction
};
use crossbeam::channel;
use log::{debug, warn};
use std::{
    sync::{Arc, Mutex},
    thread, time::SystemTime,
};

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
}

pub fn new(
    num_worker: usize,
    msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
) -> Context {
    Context {
        msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        blockchain,
    }
}

impl Context {
    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&self) {
        loop {
            let msg = self.msg_chan.recv().unwrap();
            let (msg, peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewBlockHashes(new_block_hashes) => {
                    debug!("NewBlockHashes: {:?}", new_block_hashes);
                    let blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let unknown_hashes: Vec<H256> = new_block_hashes
                        .into_iter()
                        .filter(|new_hash| blockchain.look_up_block(new_hash).is_none())
                        .collect();
                    drop(blockchain);
                    if !unknown_hashes.is_empty() {
                        peer.write(Message::GetBlocks(unknown_hashes));
                    }
                }
                Message::GetBlocks(requested_block_hashes) => {
                    debug!("GetBlocks: {:?}", requested_block_hashes);
                    let blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let requested_blocks: Vec<Block> = requested_block_hashes
                        .into_iter()
                        .filter_map(|hash| blockchain.look_up_block(&hash))
                        .map(|(block, _, _)| block.clone())
                        .collect();
                    drop(blockchain);
                    if !requested_blocks.is_empty() {
                        peer.write(Message::Blocks(requested_blocks));
                    }
                }
                Message::Blocks(blocks) => {
                    debug!("Blocks: {:?}", blocks.iter().map(Block::hash).collect::<Vec<_>>());
                    let now: u128 = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .expect("system time should always be after Unix epoch")
                        .as_millis();
                    let mut blockchain =
                        self.blockchain.lock().expect("idk why this should succeed");
                    let mut all_added_blocks = vec![];
                    for block in blocks {
                        let latency = now - block.header.timestamp;
                        let mut added_blocks = blockchain.insert_block_with_validation(block);
                        all_added_blocks.append(&mut added_blocks);
                    }
                    if !all_added_blocks.is_empty() {
                        self.server.broadcast(Message::NewBlockHashes(all_added_blocks));
                    }
                }
                Message::NewTransactionHashes(new_transaction_hashes) => {
                    debug!("NewTransactionHashes: {:?}", new_transaction_hashes);
                    let blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let unknown_hashes: Vec<H256> = new_transaction_hashes
                        .into_iter()
                        .filter(|new_hash| blockchain.get_transaction(new_hash).is_none())
                        .collect();
                    drop(blockchain);
                    if !unknown_hashes.is_empty() {
                        peer.write(Message::GetTransactions(unknown_hashes));
                    }
                }
                Message::GetTransactions(requested_hashes) => {
                    debug!("GetTransactions: {:?}", requested_hashes);
                    let blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let requested_transactions: Vec<Transaction> = requested_hashes
                        .into_iter()
                        .filter_map(|hash| blockchain.get_transaction(&hash))
                        .map(|transaction| transaction.clone())
                        .collect();
                    drop(blockchain);
                    if !requested_transactions.is_empty() {
                        peer.write(Message::Transactions(requested_transactions));
                    }
                }
                Message::Transactions(transactions) => {
                    debug!("Transactions: {:?}", transactions.iter().map(Transaction::hash).collect::<Vec<_>>());
                    let mut blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let mut all_added_transactions = vec![];
                    for transaction in transactions {
                        let hash = transaction.hash();
                        if blockchain.insert_transaction_with_validation(transaction) {
                            all_added_transactions.push(hash);
                        }
                    }
                    if !all_added_transactions.is_empty() {
                        self.server.broadcast(Message::NewTransactionHashes(all_added_transactions));
                    }
                }
            }
        }
    }
}
