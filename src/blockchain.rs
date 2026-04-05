use crate::types::*;
use crate::database::Database;
use crate::block_utils::BlockUtils;
use crate::crypto::CryptoUtils;
use anyhow::{Result, anyhow};
use std::sync::{Arc, Mutex};
use chrono::Utc;

pub struct Blockchain {
    db: Arc<Database>,
    pending_transactions: Arc<Mutex<Vec<Transaction>>>,
    mining_in_progress: Arc<Mutex<bool>>,
    total_supply: f64,
    initial_reward: f64,
    halving_interval: u64,
    max_transactions_per_block: usize,
}

impl Blockchain {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            pending_transactions: Arc::new(Mutex::new(Vec::new())),
            mining_in_progress: Arc::new(Mutex::new(false)),
            total_supply: 100_000_000.0,
            initial_reward: 1000.0,
            halving_interval: 60480,
            max_transactions_per_block: 100,
        }
    }

    pub async fn initialize(&self, master_wallet_address: &str) -> Result<()> {
        if self.db.get_latest_block()?.is_none() {
            let genesis = self.create_genesis_block(master_wallet_address);
            self.db.save_block(&genesis)?;
            self.db.set_balance(master_wallet_address, 0.0, "REDIPS")?;
            log::info!("Genesis block created");
        }
        Ok(())
    }

    fn create_genesis_block(&self, master_wallet: &str) -> Block {
        let transactions = Vec::new();
        let merkle_root = BlockUtils::calculate_merkle_root(&transactions);

        let mut block = Block {
            index: 0,
            timestamp: Utc::now().timestamp_millis(),
            transactions,
            previous_hash: "0".to_string(),
            merkle_root,
            hash: String::new(),
            nonce: 0,
            extra_nonce: 0,
            difficulty: BlockUtils::INITIAL_DIFFICULTY,
            miner: master_wallet.to_string(),
            reward: 0.0,
        };

        block.hash = BlockUtils::calculate_block_hash(&block);
        block
    }

    pub async fn add_transaction(&self, transaction: Transaction) -> Result<String> {
        self.validate_transaction(&transaction).await?;

        self.pending_transactions.lock().unwrap().push(transaction.clone());
        self.db.save_transaction(&transaction)?;

        Ok("Transaction added to pending pool".to_string())
    }

    async fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        if tx.from != "SYSTEM" {
            if tx.signature.is_empty() {
                return Err(anyhow!("Missing signature"));
            }

            let token_id = tx.data.as_ref()
                .and_then(|d| d.get("tokenId"))
                .and_then(|v| v.as_str())
                .unwrap_or("REDIPS");

            let balance = self.db.get_balance(&tx.from, token_id)?;
            if balance < tx.amount + tx.fee {
                return Err(anyhow!("Insufficient balance"));
            }
        }

        Ok(())
    }

    pub async fn mine_block(&self, miner_address: &str) -> Result<Block> {
        {
            let mut mining = self.mining_in_progress.lock().unwrap();
            if *mining {
                return Err(anyhow!("Mining already in progress"));
            }
            *mining = true;
        }

        let result = self.mine_block_internal(miner_address).await;

        *self.mining_in_progress.lock().unwrap() = false;

        result
    }

    async fn mine_block_internal(&self, miner_address: &str) -> Result<Block> {
        let latest_block = self.db.get_latest_block()?
            .ok_or_else(|| anyhow!("No genesis block found"))?;

        let blocks = self.db.get_all_blocks()?;
        let difficulty = BlockUtils::adjust_difficulty(&blocks);
        let reward = BlockUtils::calculate_mining_reward(
            latest_block.index + 1,
            self.initial_reward,
            self.halving_interval,
        );

        let stats = self.get_stats().await?;
        if stats.circulating_supply + reward > self.total_supply {
            return Err(anyhow!("Total supply cap reached"));
        }

        let mut pending = self.pending_transactions.lock().unwrap();
        let count = pending.len().min(self.max_transactions_per_block);
        let mut block_transactions: Vec<Transaction> = pending.drain(..count).collect();
        drop(pending);

        let reward_tx = Transaction {
            id: CryptoUtils::generate_id(),
            from: "SYSTEM".to_string(),
            to: miner_address.to_string(),
            amount: reward,
            fee: 0.0,
            timestamp: Utc::now().timestamp_millis(),
            signature: "MINING_REWARD".to_string(),
            tx_type: TransactionType::Transfer,
            data: None,
        };
        block_transactions.insert(0, reward_tx);

        let new_block = BlockUtils::mine_block(
            latest_block.index + 1,
            block_transactions,
            latest_block.hash.clone(),
            difficulty,
            miner_address.to_string(),
            reward,
            latest_block.timestamp,
        );

        if !BlockUtils::is_valid_timestamp(&new_block, &latest_block) {
            return Err(anyhow!("Block timestamp is invalid"));
        }
        if !BlockUtils::is_valid_block_hash(&new_block) {
            return Err(anyhow!("Mined block has invalid hash"));
        }

        self.process_block_transactions(&new_block).await?;
        self.db.save_block(&new_block)?;

        Ok(new_block)
    }

    async fn process_block_transactions(&self, block: &Block) -> Result<()> {
        for tx in &block.transactions {
            let token_id = tx.data.as_ref()
                .and_then(|d| d.get("tokenId"))
                .and_then(|v| v.as_str())
                .unwrap_or("REDIPS");

            if tx.from != "SYSTEM" {
                let sender_balance = self.db.get_balance(&tx.from, token_id)?;
                self.db.set_balance(&tx.from, sender_balance - tx.amount - tx.fee, token_id)?;
            }

            let receiver_balance = self.db.get_balance(&tx.to, token_id)?;
            self.db.set_balance(&tx.to, receiver_balance + tx.amount, token_id)?;

            if tx.fee > 0.0 && tx.from != "SYSTEM" {
                let master = std::env::var("MASTER_WALLET_ADDRESS")?;
                let master_balance = self.db.get_balance(&master, "REDIPS")?;
                self.db.set_balance(&master, master_balance + tx.fee, "REDIPS")?;
            }
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> Result<BlockchainStats> {
        let blocks = self.db.get_all_blocks()?;
        let latest = blocks.last().ok_or_else(|| anyhow!("No blocks found"))?;

        let total_transactions = blocks.iter().map(|b| b.transactions.len() as u64).sum();
        let circulating_supply = blocks.iter().map(|b| b.reward).sum();

        let halving_count = (blocks.len() as u64) / self.halving_interval;
        let next_halving_block = (halving_count + 1) * self.halving_interval;

        Ok(BlockchainStats {
            total_blocks: blocks.len() as u64,
            total_transactions,
            total_supply: self.total_supply,
            circulating_supply,
            genesis_timestamp: blocks[0].timestamp,
            last_block_timestamp: latest.timestamp,
            current_difficulty: latest.difficulty,
            hash_rate: self.calculate_hash_rate(&blocks),
            halving_count: halving_count as u32,
            next_halving_block,
        })
    }

    fn calculate_hash_rate(&self, blocks: &[Block]) -> f64 {
        if blocks.len() < 10 {
            return 0.0;
        }

        let recent = &blocks[blocks.len() - 10..];
        let time_span = recent.last().unwrap().timestamp - recent[0].timestamp;
        let avg_difficulty: f64 = recent.iter().map(|b| b.difficulty as f64).sum::<f64>()
            / recent.len() as f64;

        (2f64.powf(avg_difficulty) * recent.len() as f64) / (time_span as f64 / 1000.0)
    }

    pub fn get_pending_transactions(&self) -> Vec<Transaction> {
        self.pending_transactions.lock().unwrap().clone()
    }

    pub fn get_balance(&self, address: &str, token_id: &str) -> Result<f64> {
        self.db.get_balance(address, token_id)
    }
}
