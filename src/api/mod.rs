use crate::blockchain;
use crate::blockchain::Blockchain;
use crate::miner::Handle as MinerHandle;
use crate::network::message::Message;
use crate::network::server::Handle as NetworkServerHandle;
use crate::transaction_generator::TransactionGenerator;
use serde::Serialize;

use log::info;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server as HTTPServer;
use url::Url;

pub struct Server {
    handle: HTTPServer,
    miner: MinerHandle,
    network: NetworkServerHandle,
    tx_gen: Sender<()>,
    blockchain: Arc<Mutex<Blockchain>>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

macro_rules! respond_result {
    ( $req:expr, $success:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let payload = ApiResponse {
            success: $success,
            message: $message.to_string(),
        };
        let resp = Response::from_string(serde_json::to_string_pretty(&payload).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}

impl Server {
    pub fn start(addr: std::net::SocketAddr, miner: &MinerHandle, network: &NetworkServerHandle, tx_gen: Sender<()>, blockchain: Arc<Mutex<Blockchain>>) {
        let handle = HTTPServer::http(&addr).unwrap();
        let server = Self {
            handle,
            miner: miner.clone(),
            network: network.clone(),
            tx_gen,
            blockchain: blockchain.clone(),
        };
        thread::spawn(move || {
            for req in server.handle.incoming_requests() {
                let miner = server.miner.clone();
                let network = server.network.clone();
                let tx_gen = server.tx_gen.clone();
                let blockchain = blockchain.clone();
                thread::spawn(move || {
                    // a valid url requires a base
                    let base_url = Url::parse(&format!("http://{}/", &addr)).unwrap();
                    let url = match base_url.join(req.url()) {
                        Ok(u) => u,
                        Err(e) => {
                            respond_result!(req, false, format!("error parsing url: {}", e));
                            return;
                        }
                    };
                    match url.path() {
                        "/miner/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let lambda = match params.get("lambda") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing lambda");
                                    return;
                                }
                            };
                            let lambda = match lambda.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing lambda: {}", e)
                                    );
                                    return;
                                }
                            };
                            miner.start(lambda);
                            respond_result!(req, true, "ok");
                        }
                        "/miner/exit" => {
                            miner.exit();
                            respond_result!(req, true, "ok");
                        }
                        "/tx_gen" => {
                            // start the transaction generator
                            let _ = tx_gen.send(());
                            respond_result!(req, true, "ok");
                        }
                        "/status" => {
                            let blockchain = blockchain.lock().expect("should work");
                            let response = format!("{}", blockchain);
                            let content_type = "Content-Type: text/plain".parse::<Header>().unwrap();
                            let resp = Response::from_string(response).with_header(content_type);
                            req.respond(resp).unwrap();
                        }
                        "/network/ping" => {
                            network.broadcast(Message::Ping(String::from("Test ping")));
                            respond_result!(req, true, "ok");
                        }
                        _ => {
                            let content_type =
                                "Content-Type: application/json".parse::<Header>().unwrap();
                            let payload = ApiResponse {
                                success: false,
                                message: "endpoint not found".to_string(),
                            };
                            let resp = Response::from_string(
                                serde_json::to_string_pretty(&payload).unwrap(),
                            )
                            .with_header(content_type)
                            .with_status_code(404);
                            req.respond(resp).unwrap();
                        }
                    }
                });
            }
        });
        info!("API server listening at {}", &addr);
    }
}
