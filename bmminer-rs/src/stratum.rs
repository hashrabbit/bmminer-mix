//! Stratum protocol client for Bitcoin pool communication
//!
//! This implements the Stratum mining protocol (v1) used by most pools.
//!
//! ## Protocol Flow
//!
//! 1. Connect to pool via TCP
//! 2. Send mining.subscribe
//! 3. Send mining.authorize (username/password)
//! 4. Receive mining.notify (new work)
//! 5. Submit shares via mining.submit
//! 6. Handle mining.set_difficulty

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Stratum JSON-RPC request
#[derive(Serialize, Debug)]
struct StratumRequest {
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

/// Stratum JSON-RPC response
#[derive(Deserialize, Debug)]
struct StratumResponse {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
    method: Option<String>,
    params: Option<Vec<serde_json::Value>>,
}

/// Mining job from pool
#[derive(Debug, Clone)]
pub struct MiningJob {
    /// Job ID from pool
    pub job_id: String,

    /// Previous block hash (32 bytes hex)
    pub prev_hash: String,

    /// Coinbase part 1 (before extranonce)
    pub coinbase1: String,

    /// Coinbase part 2 (after extranonce)
    pub coinbase2: String,

    /// Merkle branch hashes
    pub merkle_branches: Vec<String>,

    /// Block version
    pub version: u32,

    /// Network difficulty bits
    pub nbits: u32,

    /// Network time
    pub ntime: u32,

    /// Clean jobs flag (discard old work)
    pub clean_jobs: bool,
}

/// Share to submit to pool
#[derive(Debug)]
pub struct Share {
    pub job_id: String,
    pub extranonce2: String,
    pub ntime: String,
    pub nonce: u32,
}

/// Stratum client
pub struct StratumClient {
    /// TCP connection to pool
    stream: Option<TcpStream>,

    /// Request ID counter
    request_id: u64,

    /// Pool URL
    url: String,

    /// Pool port
    port: u16,

    /// Worker username
    username: String,

    /// Worker password
    password: String,

    /// Extranonce1 from pool
    extranonce1: Option<String>,

    /// Extranonce2 size
    extranonce2_size: usize,

    /// Current difficulty
    difficulty: f64,

    /// Channel for receiving jobs
    job_tx: mpsc::UnboundedSender<MiningJob>,
}

impl StratumClient {
    /// Create new Stratum client
    pub fn new(
        url: String,
        port: u16,
        username: String,
        password: String,
    ) -> (Self, mpsc::UnboundedReceiver<MiningJob>) {
        let (job_tx, job_rx) = mpsc::unbounded_channel();

        let client = Self {
            stream: None,
            request_id: 0,
            url,
            port,
            username,
            password,
            extranonce1: None,
            extranonce2_size: 0,
            difficulty: 1.0,
            job_tx,
        };

        (client, job_rx)
    }

    /// Connect to pool
    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.url, self.port);
        let stream = TcpStream::connect(&addr).await?;

        println!("Connected to pool: {}", addr);

        self.stream = Some(stream);
        Ok(())
    }

    /// Subscribe to mining
    async fn subscribe(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let request = StratumRequest {
            id: self.next_id(),
            method: "mining.subscribe".to_string(),
            params: vec![
                serde_json::json!("bmminer-rs/0.1.0"),
            ],
        };

        let response = self.send_request(request).await?;

        // Parse extranonce1 and extranonce2_size from response
        if let Some(result) = response.result {
            if let Some(arr) = result.as_array() {
                if arr.len() >= 2 {
                    self.extranonce1 = arr[1].as_str().map(|s| s.to_string());
                    self.extranonce2_size = arr[2].as_u64().unwrap_or(4) as usize;
                }
            }
        }

        println!("Subscribed: extranonce1={:?}, extranonce2_size={}",
                 self.extranonce1, self.extranonce2_size);

        Ok(())
    }

    /// Authorize worker
    async fn authorize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let request = StratumRequest {
            id: self.next_id(),
            method: "mining.authorize".to_string(),
            params: vec![
                serde_json::json!(self.username),
                serde_json::json!(self.password),
            ],
        };

        let response = self.send_request(request).await?;

        if let Some(result) = response.result {
            if result.as_bool() == Some(true) {
                println!("Authorized as: {}", self.username);
                Ok(())
            } else {
                Err("Authorization failed".into())
            }
        } else {
            Err("No authorization response".into())
        }
    }

    /// Submit share to pool
    pub async fn submit_share(&mut self, share: Share) -> Result<bool, Box<dyn std::error::Error>> {
        let request = StratumRequest {
            id: self.next_id(),
            method: "mining.submit".to_string(),
            params: vec![
                serde_json::json!(self.username),
                serde_json::json!(share.job_id),
                serde_json::json!(share.extranonce2),
                serde_json::json!(share.ntime),
                serde_json::json!(format!("{:08x}", share.nonce)),
            ],
        };

        let response = self.send_request(request).await?;

        Ok(response.result.and_then(|r| r.as_bool()).unwrap_or(false))
    }

    /// Main event loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Connect and authenticate
        self.connect().await?;
        self.subscribe().await?;
        self.authorize().await?;

        // Read loop
        let stream = self.stream.take().unwrap();
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;

            if n == 0 {
                println!("Connection closed by pool");
                break;
            }

            // Parse JSON-RPC message
            if let Ok(msg) = serde_json::from_str::<StratumResponse>(&line) {
                self.handle_message(msg).await?;
            }
        }

        Ok(())
    }

    /// Handle incoming Stratum message
    async fn handle_message(&mut self, msg: StratumResponse) -> Result<(), Box<dyn std::error::Error>> {
        // Check if it's a notification (method present)
        if let Some(method) = msg.method {
            match method.as_str() {
                "mining.notify" => {
                    if let Some(params) = msg.params {
                        let job = self.parse_job(params)?;
                        println!("New job: {} (clean={})", job.job_id, job.clean_jobs);
                        self.job_tx.send(job)?;
                    }
                }
                "mining.set_difficulty" => {
                    if let Some(params) = msg.params {
                        if let Some(diff) = params.get(0).and_then(|v| v.as_f64()) {
                            self.difficulty = diff;
                            println!("Difficulty set to: {}", diff);
                        }
                    }
                }
                _ => {
                    println!("Unknown method: {}", method);
                }
            }
        }

        Ok(())
    }

    /// Parse mining.notify params into MiningJob
    fn parse_job(&self, params: Vec<serde_json::Value>) -> Result<MiningJob, Box<dyn std::error::Error>> {
        if params.len() < 9 {
            return Err("Invalid job params".into());
        }

        Ok(MiningJob {
            job_id: params[0].as_str().unwrap_or("").to_string(),
            prev_hash: params[1].as_str().unwrap_or("").to_string(),
            coinbase1: params[2].as_str().unwrap_or("").to_string(),
            coinbase2: params[3].as_str().unwrap_or("").to_string(),
            merkle_branches: params[4]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            version: u32::from_str_radix(params[5].as_str().unwrap_or("00000000"), 16).unwrap_or(0),
            nbits: u32::from_str_radix(params[6].as_str().unwrap_or("00000000"), 16).unwrap_or(0),
            ntime: u32::from_str_radix(params[7].as_str().unwrap_or("00000000"), 16).unwrap_or(0),
            clean_jobs: params[8].as_bool().unwrap_or(false),
        })
    }

    /// Send request and wait for response
    async fn send_request(&mut self, request: StratumRequest) -> Result<StratumResponse, Box<dyn std::error::Error>> {
        let stream = self.stream.as_mut().ok_or("Not connected")?;

        let json = serde_json::to_string(&request)?;
        stream.write_all(json.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        // For now, return dummy response
        // In real implementation, need to match request ID with response
        Ok(StratumResponse {
            id: Some(request.id),
            result: Some(serde_json::json!(true)),
            error: None,
            method: None,
            params: None,
        })
    }

    /// Get next request ID
    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }
}
