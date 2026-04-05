use sled::Db;
use crate::types::*;
use anyhow::Result;
use std::path::Path;

pub struct Database {
    db: Db,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn save_block(&self, block: &Block) -> Result<()> {
        let key = format!("block:{}", block.index);
        self.db.insert(key.as_bytes(), serde_json::to_vec(block)?)?;
        Ok(())
    }

    pub fn get_block(&self, index: u64) -> Result<Option<Block>> {
        let key = format!("block:{}", index);
        match self.db.get(key.as_bytes())? {
            Some(data) => Ok(Some(serde_json::from_slice(&data)?)),
            None => Ok(None),
        }
    }

    pub fn get_latest_block(&self) -> Result<Option<Block>> {
        Ok(self.get_all_blocks()?.into_iter().last())
    }

    pub fn get_all_blocks(&self) -> Result<Vec<Block>> {
        let mut blocks: Vec<Block> = self
            .db
            .scan_prefix(b"block:")
            .map(|item| {
                let (_, value) = item?;
                Ok(serde_json::from_slice(&value)?)
            })
            .collect::<Result<_>>()?;

        blocks.sort_by_key(|b| b.index);
        Ok(blocks)
    }

    pub fn save_transaction(&self, tx: &Transaction) -> Result<()> {
        let key = format!("tx:{}", tx.id);
        self.db.insert(key.as_bytes(), serde_json::to_vec(tx)?)?;
        Ok(())
    }

    pub fn get_transaction(&self, id: &str) -> Result<Option<Transaction>> {
        let key = format!("tx:{}", id);
        match self.db.get(key.as_bytes())? {
            Some(data) => Ok(Some(serde_json::from_slice(&data)?)),
            None => Ok(None),
        }
    }

    pub fn get_balance(&self, address: &str, token_id: &str) -> Result<f64> {
        let key = format!("balance:{}:{}", address, token_id);
        match self.db.get(key.as_bytes())? {
            Some(data) => Ok(String::from_utf8(data.to_vec())?.parse()?),
            None => Ok(0.0),
        }
    }

    pub fn set_balance(&self, address: &str, amount: f64, token_id: &str) -> Result<()> {
        let key = format!("balance:{}:{}", address, token_id);
        self.db.insert(key.as_bytes(), amount.to_string().as_bytes())?;
        Ok(())
    }
}
