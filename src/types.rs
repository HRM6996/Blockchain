use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub fee: f64,
    pub timestamp: i64,
    pub signature: String,
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub data: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    Transfer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    pub merkle_root: String,
    pub nonce: u64,
    pub extra_nonce: u64,
    pub difficulty: u32,
    pub miner: String,
    pub reward: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub address: String,
    pub public_key: String,
    pub private_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub total_blocks: u64,
    pub total_transactions: u64,
    pub total_supply: f64,
    pub circulating_supply: f64,
    pub genesis_timestamp: i64,
    pub last_block_timestamp: i64,
    pub current_difficulty: u32,
    pub hash_rate: f64,
    pub halving_count: u32,
    pub next_halving_block: u64,
}
