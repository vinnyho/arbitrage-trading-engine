use tokio::sync::mpsc;
use crate::types::{ArbSignal, Exchange};
use rsa::RsaPrivateKey;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest;

pub async fn run(
    mut rx: mpsc::UnboundedReceiver<ArbSignal>,
    key_id: String,
    private_key: Arc<RsaPrivateKey>,
) -> Result<(), anyhow::Error >{
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

            println!("{} {:?}", response.status(), response.text().await);
        }
        
    }
    Ok(())
}
