mod kalshi;
mod types;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::types::OrderBook;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    let kalshi_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));


    if let Err(e) = kalshi::connect(&kalshi_books).await {
        println!("error: {}", e);
    }
    
}