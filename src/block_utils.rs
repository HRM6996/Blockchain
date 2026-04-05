use crate::types::Block;
use sha2::{Sha256, Digest};
use chrono::Utc;

pub struct BlockUtils;

impl BlockUtils {
    pub const INITIAL_DIFFICULTY: u32 = 20;
    pub const BLOCK_TIME_TARGET: i64 = 30_000;
    pub const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 144;
    pub const MIN_BLOCK_INTERVAL_MS: i64 = 5_000;
    pub const MAX_DIFFICULTY_CHANGE: u32 = 4;

    fn hash_pair(a: &[u8], b: &[u8]) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(a);
        h.update(b);
        h.finalize().to_vec()
    }

    pub fn calculate_merkle_root(transactions: &[crate::types::Transaction]) -> String {
        if transactions.is_empty() {
            return "0".repeat(64);
        }

        let mut level: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| {
                let first = Sha256::digest(tx.id.as_bytes());
                Sha256::digest(&first).to_vec()
            })
            .collect();

        while level.len() > 1 {
            if level.len() % 2 != 0 {
                let last = level.last().unwrap().clone();
                level.push(last);
            }

            level = level
                .chunks(2)
                .map(|pair| Self::hash_pair(&pair[0], &pair[1]))
                .collect();
        }

        hex::encode(&level[0])
    }

    pub fn calculate_block_hash(block: &Block) -> String {
        let data = serde_json::json!({
            "index":        block.index,
            "timestamp":    block.timestamp,
            "merkleRoot":   block.merkle_root,
            "previousHash": block.previous_hash,
            "nonce":        block.nonce,
            "extraNonce":   block.extra_nonce,
            "difficulty":   block.difficulty,
            "miner":        block.miner,
        });

        let first_pass = Sha256::digest(data.to_string().as_bytes());
        let second_pass = Sha256::digest(&first_pass);
        hex::encode(second_pass)
    }

    pub fn hash_meets_target(hash_hex: &str, difficulty: u32) -> bool {
        let hash_bytes = match hex::decode(hash_hex) {
            Ok(b) if b.len() == 32 => b,
            _ => return false,
        };

        let difficulty = difficulty.min(255) as usize;

        let full_bytes = difficulty / 8;
        let remaining_bits = difficulty % 8;

        for i in 0..full_bytes {
            if hash_bytes[i] != 0 {
                return false;
            }
        }

        if remaining_bits > 0 {
            let mask = 0xFF_u8 >> remaining_bits;
            if hash_bytes[full_bytes] > mask {
                return false;
            }
        }

        true
    }

    pub fn mine_block(
        index: u64,
        transactions: Vec<crate::types::Transaction>,
        previous_hash: String,
        difficulty: u32,
        miner_address: String,
        reward: f64,
        previous_timestamp: i64,
    ) -> Block {
        let merkle_root = Self::calculate_merkle_root(&transactions);

        let earliest_allowed = previous_timestamp + Self::MIN_BLOCK_INTERVAL_MS;
        let now = Utc::now().timestamp_millis();
        let timestamp = now.max(earliest_allowed);

        let mut nonce: u64 = 0;
        let mut extra_nonce: u64 = 0;

        loop {
            let mut block = Block {
                index,
                timestamp,
                transactions: transactions.clone(),
                previous_hash: previous_hash.clone(),
                merkle_root: merkle_root.clone(),
                hash: String::new(),
                nonce,
                extra_nonce,
                difficulty,
                miner: miner_address.clone(),
                reward,
            };

            block.hash = Self::calculate_block_hash(&block);

            if Self::hash_meets_target(&block.hash, difficulty) {
                return block;
            }

            if nonce == u64::MAX {
                nonce = 0;
                extra_nonce = extra_nonce.wrapping_add(1);
            } else {
                nonce += 1;
            }
        }
    }

    pub fn is_valid_block_hash(block: &Block) -> bool {
        let calculated = Self::calculate_block_hash(block);
        calculated == block.hash && Self::hash_meets_target(&block.hash, block.difficulty)
    }

    pub fn is_valid_timestamp(block: &Block, previous_block: &Block) -> bool {
        let now = Utc::now().timestamp_millis();

        let after_min_interval =
            block.timestamp >= previous_block.timestamp + Self::MIN_BLOCK_INTERVAL_MS;

        let not_from_future = block.timestamp <= now + 120_000;

        after_min_interval && not_from_future
    }

    pub fn adjust_difficulty(blocks: &[Block]) -> u32 {
        if blocks.len() < Self::DIFFICULTY_ADJUSTMENT_INTERVAL as usize {
            return Self::INITIAL_DIFFICULTY;
        }

        let last_adj_idx = blocks.len() - Self::DIFFICULTY_ADJUSTMENT_INTERVAL as usize;
        let last_adj_block = &blocks[last_adj_idx];
        let last_block = blocks.last().unwrap();

        let time_expected =
            Self::BLOCK_TIME_TARGET * Self::DIFFICULTY_ADJUSTMENT_INTERVAL as i64;
        let time_actual = last_block.timestamp - last_adj_block.timestamp;

        let current = last_block.difficulty;

        let ideal = if time_actual == 0 {
            current + Self::MAX_DIFFICULTY_CHANGE
        } else {
            let ratio = time_expected as f64 / time_actual as f64;
            let bit_adjust = ratio.log2();
            if bit_adjust >= 0.0 {
                current.saturating_add(bit_adjust as u32)
            } else {
                current.saturating_sub((-bit_adjust) as u32)
            }
        };

        let clamped = if ideal > current {
            current + (ideal - current).min(Self::MAX_DIFFICULTY_CHANGE)
        } else {
            current - (current - ideal).min(Self::MAX_DIFFICULTY_CHANGE)
        };

        clamped.max(Self::INITIAL_DIFFICULTY)
    }

    pub fn calculate_mining_reward(
        block_index: u64,
        initial_reward: f64,
        halving_interval: u64,
    ) -> f64 {
        let halvings = block_index / halving_interval;
        initial_reward / 2f64.powi(halvings as i32)
    }
}
