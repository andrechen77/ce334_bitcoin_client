use crate::block::{Block, Content, Header};
use crate::blockchain::Blockchain;
use crate::crypto::hash::Hashable;
use crate::crypto::merkle::MerkleTree;
use crate::mempool::Mempool;
use crate::network::message::Message;
use crate::network::server::Handle as ServerHandle;
use crate::transaction;

use log::{debug, info};

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use std::{iter, thread};

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<Mempool>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(server: &ServerHandle, blockchain: Arc<Mutex<Blockchain>>, mempool: Arc<Mutex<Mempool>>) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        blockchain,
        mempool,
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn handle_control_signal(&mut self, signal: ControlSignal) {
        match signal {
            ControlSignal::Exit => {
                info!("Miner shutting down");
                self.operating_state = OperatingState::ShutDown;
            }
            ControlSignal::Start(i) => {
                info!("Miner starting in continuous mode with lambda {}", i);
                self.operating_state = OperatingState::Run(i);
            }
        }
    }

    fn miner_loop(&mut self) {
        // create a block
        let mut block = self.create_next_block(rand::random());

        // main mining loop
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    self.handle_control_signal(signal);
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        self.handle_control_signal(signal);
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // do one iteration of mining
            block.header.timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system time should always be after Unix epoch")
                .as_millis();
            let hash = block.hash();
            if hash <= block.header.difficulty {
                // add the block to the chain
                let mut blockchain = self.blockchain.lock().expect("idk why this should succeed");
                blockchain.insert(block);
                drop(blockchain);
                info!("Mined a block! Added to blockchain");
                self.server.broadcast(Message::NewBlockHashes(vec![hash]));
                block = self.create_next_block(rand::random());
            } else {
                debug!("Didn't work, trying another nonce");
                // increment the nonce for the next iteration
                block.header.nonce += 1;
                // should never wrap back around to the starting nonce
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }

    fn create_next_block(&self, starting_nonce: u32) -> Block {
        debug!("Creating the next block");
        let blockchain = self.blockchain.lock().expect("idk why this should be safe");
        let parent_hash = blockchain.tip();
        let (parent_block, _, _) = blockchain
            .look_up_block(&parent_hash)
            .expect("parent of tip should exist");
        let difficulty = parent_block.header.difficulty;
        drop(blockchain);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should always be after Unix epoch")
            .as_millis();
        let transactions = Vec::from_iter(
            iter::repeat_with(|| transaction::SignedTransaction::generate_random()).take(10),
        );
        let merkle_tree = MerkleTree::new(&transactions);
        let merkle_root = merkle_tree.root();
        Block {
            header: Header {
                parent: parent_hash,
                nonce: starting_nonce,
                difficulty,
                timestamp,
                merkle_root,
            },
            content: Content { transactions },
        }
    }
}
