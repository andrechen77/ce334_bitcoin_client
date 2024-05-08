use super::message::Message;
use super::peer;
use crate::{
    block::Block,
    blockchain::Blockchain,
    crypto::hash::{Hashable, H256},
    network::server::Handle as ServerHandle,
};
use crossbeam::channel;
use log::{debug, warn};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
};

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    // TODO mempool
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
                Message::NewBlockHashes(new_hashes) => {
                    debug!("NewBlockHashes: {:?}", new_hashes);
                    let blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    let unknown_hashes: Vec<H256> = new_hashes
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
                        .map(|(block, _height)| block.clone())
                        .collect();
                    drop(blockchain);
                    if !requested_blocks.is_empty() {
                        peer.write(Message::Blocks(requested_blocks));
                    }
                }
                Message::Blocks(blocks) => {
                    debug!("Blocks: {:?}", blocks.iter().map(Block::hash).collect::<Vec<_>>());
                    let mut blockchain =
                        self.blockchain.lock().expect("idk why this should succeed");
                    let mut all_added_blocks = vec![];
                    for block in blocks {
                        let mut added_blocks = blockchain.insert_with_validation(block);
                        all_added_blocks.append(&mut added_blocks);
                    }
                    if !all_added_blocks.is_empty() {
                        self.server.broadcast(Message::NewBlockHashes(all_added_blocks));
                    }
                }
            }
        }
    }
}
