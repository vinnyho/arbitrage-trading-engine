use crate::types::{ArbSignal, Exchange};
use chrono::Utc;
use reqwest;
use rsa::RsaPrivateKey;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::poly_sign;
use tokio::sync::mpsc;

pub struct PolyKeys {
    pub address: String,
    pub funder: String,
    pub private_key: String,
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

async fn execute_kalshi_trade(
    client: &reqwest::Client,
    key_id: &str,
    private_key: &Arc<RsaPrivateKey>,
    signal: &ArbSignal,
) -> Result<String, anyhow::Error> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let msg = format!("{}POST/trade-api/v2/portfolio/orders", timestamp);
    let signature = crate::kalshi::sign(private_key, &msg)?;

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
        .header("KALSHI-ACCESS-KEY", key_id)
        .header("KALSHI-ACCESS-SIGNATURE", &signature)
        .header("KALSHI-ACCESS-TIMESTAMP", &timestamp)
        .json(&body)
        .send()
        .await?;

    Ok(format!(
        "{} {}",
        response.status(),
        response.text().await.unwrap_or_default()
    ))
}

async fn execute_polymarket_trade(
    client: &reqwest::Client,
    keys: &PolyKeys,
    signal: &ArbSignal,
) -> Result<String, anyhow::Error> {
    let maker_amount = (signal.buy_price * signal.size * 1_000_000.0).round() as u64;
    let taker_amount = (signal.size * 1_000_000.0).round() as u64;

    let timestamp_ms: u64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let timestamp_secs = (timestamp_ms / 1000).to_string();

    let (signature, salt) = poly_sign::sign_order(
        &keys.private_key,
        &keys.funder,
        &signal.polymarket_token_id,
        maker_amount,
        taker_amount,
        timestamp_ms,
    )?;

    let body = serde_json::json!({
        "order": {
            "salt": salt,
            "maker": keys.funder,
            "signer": keys.funder,
            "tokenId": signal.polymarket_token_id,
            "makerAmount": maker_amount.to_string(),
            "takerAmount": taker_amount.to_string(),
            "expiration": "0",
            "side": "BUY",
            "signatureType": poly_sign::SIGNATURE_TYPE_POLY_1271,
            "timestamp": timestamp_ms.to_string(),
            "metadata": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "builder": format!("0x{}", poly_sign::BUILDER_HEX),
            "signature": signature,
        },
        "owner": keys.api_key,
        "orderType": "GTC",
        "deferExec": false,
        "postOnly": false,
    });

    let body_str = body.to_string();
    let l2_sig = poly_sign::l2_signature(&keys.secret, &timestamp_secs, "POST", "/order", &body_str)?;

    let response = client
        .post("https://clob.polymarket.com/order")
        .header("POLY_ADDRESS", &keys.address)
        .header("POLY_API_KEY", &keys.api_key)
        .header("POLY_PASSPHRASE", &keys.passphrase)
        .header("POLY_TIMESTAMP", &timestamp_secs)
        .header("POLY_SIGNATURE", l2_sig)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await?;

    Ok(format!(
        "{} {}",
        response.status(),
        response.text().await.unwrap_or_default()
    ))
}

pub async fn run(
    mut rx: mpsc::UnboundedReceiver<ArbSignal>,
    key_id: String,
    private_key: Arc<RsaPrivateKey>,
    poly: PolyKeys,
    mut db_conn: rusqlite::Connection,
) -> Result<(), anyhow::Error> {
    let mut live_trades: HashSet<String> = HashSet::new();
    let client = reqwest::Client::new();

    while let Some(signal) = rx.recv().await {
        println!("{:?}", signal);
        if live_trades.contains(&signal.canonical_id) {
            continue;
        }
        live_trades.insert(signal.canonical_id.clone());

        let result = match signal.buy_exchange {
            Exchange::Kalshi => execute_kalshi_trade(&client, &key_id, &private_key, &signal).await,
            Exchange::Polymarket => execute_polymarket_trade(&client, &poly, &signal).await,
        };

        let status = match result {
            Ok(s) => {
                println!("{}", s);
                s
            }
            Err(e) => {
                println!("trade error: {}", e);
                format!("error: {}", e)
            }
        };

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
    Ok(())
}
