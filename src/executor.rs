use crate::types::{ArbSignal, Exchange};
use chrono::Utc;
use reqwest;
use rsa::RsaPrivateKey;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub async fn run(
    mut rx: mpsc::UnboundedReceiver<ArbSignal>,
    key_id: String,
    private_key: Arc<RsaPrivateKey>,
    mut db_conn: rusqlite::Connection,
) -> Result<(), anyhow::Error> {
    let mut live_trades: HashSet<String> = HashSet::new();
    let client = reqwest::Client::new();

    while let Some(signal) = rx.recv().await {
        println!("{:?}", signal);
        if !live_trades.contains(&signal.canonical_id) && signal.buy_exchange == Exchange::Kalshi {
            live_trades.insert(signal.canonical_id.clone());

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string();
            let msg = format!("{}POST/trade-api/v2/portfolio/orders", timestamp);
            let signature = crate::kalshi::sign(&private_key, &msg)?;

            let body = serde_json::json!({
                "ticker": signal.kalshi_ticker,
                "client_order_id": format!("{}-{}", signal.canonical_id, timestamp),
                "side": "yes",
                "action": "buy",
                "count": signal.size as i32,
                "type": "limit",
                "yes_price": (signal.buy_price * 100.0).round() as i32,
            });

            let response = client
                .post("https://external-api.kalshi.com/trade-api/v2/portfolio/orders")
                .header("KALSHI-ACCESS-KEY", &key_id)
                .header("KALSHI-ACCESS-SIGNATURE", &signature)
                .header("KALSHI-ACCESS-TIMESTAMP", &timestamp)
                .json(&body)
                .send()
                .await?;

            let status = response.status().to_string();
            let body = response.text().await.unwrap_or_default();
            println!("{} {}", status, body);

            let canonical_id = signal.canonical_id.clone();
            let ticker = signal.kalshi_ticker.clone();
            let price = signal.buy_price;
            let count = signal.size as i32;
            let created_at = Utc::now().to_rfc3339();

            db_conn = tokio::task::spawn_blocking(move || {
                db_conn
                    .execute(
                        "INSERT INTO trades (canonical_id, ticker, price, count, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        rusqlite::params![canonical_id, ticker, price, count, status, created_at],
                    )
                    .ok();
                db_conn
            })
            .await?;
        }
    }
    Ok(())
}
