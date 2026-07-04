use crate::types::OrderBook;
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use rand::thread_rng;
use rsa::signature::SignatureEncoding;
use rsa::{RsaPrivateKey, pss::BlindedSigningKey, signature::RandomizedSigner};
use serde::Deserialize;
use serde_json;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize)]
struct KalshiTicker {
    #[serde(rename = "type")]
    msg_type: String,
    msg: Option<KalshiData>,
}
#[derive(Debug, Deserialize)]
struct KalshiData {
    market_ticker: String,
    yes_bid_dollars: String,
    yes_ask_dollars: String,
}
pub fn sign(private_key: &RsaPrivateKey, msg: &str) -> Result<String, anyhow::Error> {
    let signing_key = BlindedSigningKey::<Sha256>::new(private_key.clone());
    let mut rng = thread_rng();

    let signature = signing_key.sign_with_rng(&mut rng, msg.as_bytes());

    Ok(STANDARD.encode(signature.to_bytes()))
}

pub async fn connect(
    books: &Arc<Mutex<HashMap<String, OrderBook>>>,
    key_id: &str,
    private_key: &RsaPrivateKey,
) -> Result<(), anyhow::Error> {
    let timestamp: String = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();

    let msg: String = format!("{}GET/trade-api/ws/v2", timestamp);
    let signature: String = sign(private_key, &msg)?;

    let mut request: tokio_tungstenite::tungstenite::http::Request<()> =
        "wss://external-api-ws.kalshi.com/trade-api/ws/v2".into_client_request()?;

    request
        .headers_mut()
        .insert("KALSHI-ACCESS-KEY", key_id.parse().unwrap());
    request
        .headers_mut()
        .insert("KALSHI-ACCESS-SIGNATURE", signature.parse().unwrap());
    request
        .headers_mut()
        .insert("KALSHI-ACCESS-TIMESTAMP", timestamp.parse().unwrap());

    let (connection, _) = connect_async(request).await?;
    let (mut write, mut read) = connection.split();

    let subscription = serde_json::json!({
        "id": 1,
        "cmd": "subscribe",
        "params": {
            "channels": ["ticker"]
        }
    });
    write
        .send(Message::Text(subscription.to_string().into()))
        .await?;

    while let Some(msg) = read.next().await {
        if let Ok(Message::Text(text)) = msg {
            match serde_json::from_str::<KalshiTicker>(&text) {
                Ok(parsed) => {
                    if let Some(data) = parsed.msg {
                        let ticket = data.market_ticker;
                        let mut books = books.lock().await;
                        let yes_bid_dollars = data.yes_bid_dollars;
                        let yes_ask_dollars = data.yes_ask_dollars;
                        books.insert(
                            ticket.clone(),
                            OrderBook {
                                market_id: ticket,
                                best_bid: yes_bid_dollars.parse::<f64>().ok(),
                                best_ask: yes_ask_dollars.parse::<f64>().ok(),
                            },
                        );
                    }
                }
                Err(_) => {}
            }
        }
    }
    Ok(())
}
