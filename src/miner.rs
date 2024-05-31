use crate::block::{Block, Content, Header};
use crate::blockchain::Blockchain;
use crate::crypto::hash::Hashable;
use crate::crypto::merkle::MerkleTree;
use crate::network::message::Message;
use crate::network::server::Handle as ServerHandle;
use crate::transaction;

use log::{debug, info, trace};

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use std::thread::current;
use std::time::{Duration, SystemTime};

use std::{iter, thread};

const OUR_MINIMUM_BLOCK_SIZE: usize = 5;
const OUR_MAXIMUM_BLOCK_SIZE: usize = 7;

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
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(server: &ServerHandle, blockchain: Arc<Mutex<Blockchain>>) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        blockchain,
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
        let mut current_block = None;

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
            // make sure we have a block to work on
            if current_block.is_none() {
                current_block = self.create_next_block(rand::random());
            }
            if let Some(block) = &mut current_block {
                block.header.timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("system time should always be after Unix epoch")
                    .as_millis();
                let hash = block.hash();
                if hash <= block.header.difficulty {
                    // add the block to the chain
                    let mut blockchain = self.blockchain.lock().expect("idk why this should succeed");
                    blockchain.insert_block_with_validation(current_block.take().expect("should exist"));
                    drop(blockchain);
                    info!("Mined a block! Added to blockchain");
                    self.server.broadcast(Message::NewBlockHashes(vec![hash]));
                } else {
                    debug!("Didn't work, trying another nonce");
                    // increment the nonce for the next iteration
                    block.header.nonce += 1;
                    // should never wrap back around to the starting nonce
                }
            } else {
                debug!("couldn't build a block");
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }

    fn create_next_block(&self, starting_nonce: u32) -> Option<Block> {
        let blockchain = self.blockchain.lock().expect("idk why this should be safe");
        let parent_hash = blockchain.tip_hash();
        let (parent_block, _, parent_state) = blockchain.tip_data();
        let difficulty = parent_block.header.difficulty;

        // attempt to build a block from the transactions in the mempool
        let mut transactions = Vec::new();
        let mut state = parent_state.clone();
        for (_, transaction) in blockchain.mempool_transactions() {
            if transactions.len() >= OUR_MAXIMUM_BLOCK_SIZE {
                break;
            }

            if state.update_in_place(&transaction.raw_transaction) {
                transactions.push(transaction);
            // } else {
            //     debug!("rejected tx: {:?}", &transaction);
            }
        }
        if transactions.len() < OUR_MINIMUM_BLOCK_SIZE {
            // unable to build a block
            return None;
        }

        // we have the transactions, now put them together into a block
        debug!("Creating the next block!");
        let transactions: Vec<_> = transactions.into_iter().map(|tx| tx.clone()).collect();
        drop(blockchain);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should always be after Unix epoch")
            .as_millis();
        let merkle_tree = MerkleTree::new(&transactions);
        let merkle_root = merkle_tree.root();
        Some(Block {
            header: Header {
                parent: parent_hash,
                nonce: starting_nonce,
                difficulty,
                timestamp,
                merkle_root,
            },
            content: Content { transactions },
        })
    }
}
