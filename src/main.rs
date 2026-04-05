mod types;
mod crypto;
mod block_utils;
mod database;
mod blockchain;
mod api;

use std::sync::Arc;
use dotenv::dotenv;
use anyhow::Result;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    log::info!("Starting REDIPS Blockchain Node...");

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "./data/blockchain".to_string());

    std::fs::create_dir_all(&db_path)?;

    let master_wallet_address = std::env::var("MASTER_WALLET_ADDRESS").ok();
    let master_wallet_private_key = std::env::var("MASTER_WALLET_PRIVATE_KEY").ok();

    let (master_address, _master_private_key) = if master_wallet_address.is_none() || master_wallet_private_key.is_none() {
        log::info!("Generating master wallet...");
        let mnemonic = crypto::CryptoUtils::generate_mnemonic();
        let wallet = crypto::CryptoUtils::mnemonic_to_wallet(&mnemonic)?;

        println!("\n=================================");
        println!("MASTER WALLET CREATED");
        println!("=================================");
        println!("Address: {}", wallet.address);
        println!("Private Key: {}", wallet.private_key);
        println!("Mnemonic: {}", mnemonic);
        println!("=================================");
        println!("SAVE THESE CREDENTIALS SECURELY!");
        println!("=================================\n");

        let env_content = format!(
            "\nMASTER_WALLET_ADDRESS={}\nMASTER_WALLET_PRIVATE_KEY={}\n",
            wallet.address, wallet.private_key
        );

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(".env")
        {
            use std::io::Write;
            let _ = file.write_all(env_content.as_bytes());
        }

        (wallet.address, wallet.private_key)
    } else {
        (
            master_wallet_address.unwrap(),
            master_wallet_private_key.unwrap(),
        )
    };

    let db = Arc::new(database::Database::new(&db_path)?);

    let blockchain = Arc::new(blockchain::Blockchain::new(db.clone()));
    blockchain.initialize(&master_address).await?;

    println!("\n=================================");
    println!("REDIPS Blockchain Node Started");
    println!("=================================");
    println!("API Server: http://localhost:{}", port);
    println!("Master Wallet: {}", master_address);
    println!("=================================\n");

    api::start_server(blockchain, db, port).await?;

    Ok(())
}
