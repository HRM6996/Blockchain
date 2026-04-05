use actix_web::{web, App, HttpServer, HttpResponse, Result as ActixResult};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::blockchain::Blockchain;
use crate::database::Database;
use crate::crypto::CryptoUtils;
use crate::types::*;

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }

    fn err(msg: impl Into<String>) -> Self {
        Self { success: false, data: None, error: Some(msg.into()) }
    }
}

#[derive(Deserialize)]
struct ImportWalletRequest {
    mnemonic: String,
}

#[derive(Serialize)]
struct WalletResponse {
    address: String,
    mnemonic: Option<String>,
}

#[derive(Serialize)]
struct BalanceResponse {
    balance: f64,
}

#[derive(Deserialize)]
struct MineRequest {
    #[serde(rename = "minerAddress")]
    miner_address: String,
}

#[derive(Serialize)]
struct BlocksResponse {
    blocks: Vec<Block>,
    total: usize,
}

async fn create_wallet() -> ActixResult<HttpResponse> {
    let mnemonic = CryptoUtils::generate_mnemonic();
    match CryptoUtils::mnemonic_to_wallet(&mnemonic) {
        Ok(wallet) => Ok(HttpResponse::Ok().json(ApiResponse::ok(WalletResponse {
            address: wallet.address,
            mnemonic: Some(mnemonic),
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn import_wallet(req: web::Json<ImportWalletRequest>) -> ActixResult<HttpResponse> {
    match CryptoUtils::mnemonic_to_wallet(&req.mnemonic) {
        Ok(wallet) => Ok(HttpResponse::Ok().json(ApiResponse::ok(WalletResponse {
            address: wallet.address,
            mnemonic: None,
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_balance(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    blockchain: web::Data<Arc<Blockchain>>,
) -> ActixResult<HttpResponse> {
    let address = path.into_inner();
    let token_id = query.get("tokenId").map(|s| s.as_str()).unwrap_or("REDIPS");

    match blockchain.get_balance(&address, token_id) {
        Ok(balance) => Ok(HttpResponse::Ok().json(ApiResponse::ok(BalanceResponse { balance }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_transactions(
    path: web::Path<String>,
    db: web::Data<Arc<Database>>,
) -> ActixResult<HttpResponse> {
    let address = path.into_inner();

    match db.get_all_blocks() {
        Ok(blocks) => {
            let mut txs: Vec<Transaction> = blocks
                .into_iter()
                .flat_map(|b| b.transactions)
                .filter(|tx| tx.from == address || tx.to == address)
                .collect();
            txs.reverse();
            Ok(HttpResponse::Ok().json(ApiResponse::ok(txs)))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn send_transaction(
    tx: web::Json<Transaction>,
    blockchain: web::Data<Arc<Blockchain>>,
) -> ActixResult<HttpResponse> {
    match blockchain.add_transaction(tx.into_inner()).await {
        Ok(msg) => Ok(HttpResponse::Ok().json(ApiResponse::ok(msg))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_transaction(
    path: web::Path<String>,
    db: web::Data<Arc<Database>>,
) -> ActixResult<HttpResponse> {
    let id = path.into_inner();
    match db.get_transaction(&id) {
        Ok(Some(tx)) => Ok(HttpResponse::Ok().json(ApiResponse::ok(tx))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::<()>::err("Transaction not found"))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_pending_transactions(blockchain: web::Data<Arc<Blockchain>>) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(ApiResponse::ok(blockchain.get_pending_transactions())))
}

async fn mine_block(
    req: web::Json<MineRequest>,
    blockchain: web::Data<Arc<Blockchain>>,
) -> ActixResult<HttpResponse> {
    match blockchain.mine_block(&req.miner_address).await {
        Ok(block) => Ok(HttpResponse::Ok().json(ApiResponse::ok(block))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_stats(blockchain: web::Data<Arc<Blockchain>>) -> ActixResult<HttpResponse> {
    match blockchain.get_stats().await {
        Ok(stats) => Ok(HttpResponse::Ok().json(ApiResponse::ok(stats))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_blocks(
    query: web::Query<std::collections::HashMap<String, String>>,
    db: web::Data<Arc<Database>>,
) -> ActixResult<HttpResponse> {
    let limit: usize = query.get("limit").and_then(|s| s.parse().ok()).unwrap_or(10);
    let offset: usize = query.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);

    match db.get_all_blocks() {
        Ok(mut blocks) => {
            let total = blocks.len();
            blocks.reverse();
            let paginated = blocks.into_iter().skip(offset).take(limit).collect();
            Ok(HttpResponse::Ok().json(ApiResponse::ok(BlocksResponse { blocks: paginated, total })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn get_block(
    path: web::Path<u64>,
    db: web::Data<Arc<Database>>,
) -> ActixResult<HttpResponse> {
    let index = path.into_inner();
    match db.get_block(index) {
        Ok(Some(block)) => Ok(HttpResponse::Ok().json(ApiResponse::ok(block))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::<()>::err("Block not found"))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e.to_string()))),
    }
}

async fn health() -> ActixResult<HttpResponse> {
    #[derive(Serialize)]
    struct Health { status: &'static str }
    Ok(HttpResponse::Ok().json(Health { status: "ok" }))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/api/wallet/create",                   web::post().to(create_wallet))
        .route("/api/wallet/import",                   web::post().to(import_wallet))
        .route("/api/wallet/{address}/balance",        web::get().to(get_balance))
        .route("/api/wallet/{address}/transactions",   web::get().to(get_transactions))
        .route("/api/transaction/send",                web::post().to(send_transaction))
        .route("/api/transaction/{id}",                web::get().to(get_transaction))
        .route("/api/transactions/pending",            web::get().to(get_pending_transactions))
        .route("/api/mine",                            web::post().to(mine_block))
        .route("/api/blockchain/stats",                web::get().to(get_stats))
        .route("/api/blockchain/blocks",               web::get().to(get_blocks))
        .route("/api/blockchain/block/{index}",        web::get().to(get_block))
        .route("/health",                              web::get().to(health));
}

pub async fn start_server(
    blockchain: Arc<Blockchain>,
    db: Arc<Database>,
    port: u16,
) -> std::io::Result<()> {
    log::info!("Starting API server on port {}", port);

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(blockchain.clone()))
            .app_data(web::Data::new(db.clone()))
            .configure(configure_routes)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
