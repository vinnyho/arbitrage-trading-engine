use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketPair {
    pub canonical_id: String,
    pub sport: String,
    pub game_date: String,
    pub team: String,
    pub kalshi_ticker: String,
    pub polymarket_token_id: String,
}

#[derive(Debug)]
struct KalshiRawMarket {
    ticker: String,
    yes_sub_title: String,
    event_ticker: String,
}

#[derive(Debug)]
struct PolyRawMarket {
    question: String,
    start_time: String,
    token_id: String,
    team: String,
}

async fn fetch_kalshi_raw(
    client: &reqwest::Client,
    series_ticker: &str,
) -> anyhow::Result<Vec<KalshiRawMarket>> {
    let resp = client
        .get("https://external-api.kalshi.com/trade-api/v2/markets")
        .query(&[
            ("series_ticker", series_ticker),
            ("status", "open"),
            ("limit", "1000"),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let markets = match resp["markets"].as_array() {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut result = Vec::new();
    for market in markets {
        let ticker = market["ticker"].as_str().unwrap_or("").to_string();
        let yes_sub_title = market["yes_sub_title"].as_str().unwrap_or("").to_string();
        let event_ticker = market["event_ticker"].as_str().unwrap_or("").to_string();
        result.push(KalshiRawMarket {
            ticker,
            yes_sub_title,
            event_ticker,
        });
    }

    Ok(result)
}

async fn fetch_poly_raw(
    client: &reqwest::Client,
    tag_id: &str,
) -> anyhow::Result<Vec<PolyRawMarket>> {
    let resp = client
        .get("https://gamma-api.polymarket.com/events")
        .query(&[
            ("tag_id", tag_id),
            ("active", "true"),
            ("closed", "false"),
            ("limit", "100"),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let events = match resp.as_array() {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut result = Vec::new();
    for event in events {
        let start_time = event["startTime"].as_str().unwrap_or("").to_string();
        let markets = match event["markets"].as_array() {
            Some(m) => m,
            None => continue,
        };

        for market in markets {
            if market["sportsMarketType"].as_str() != Some("moneyline") {
                continue;
            }

            let question = market["question"].as_str().unwrap_or("").to_string();
            let outcomes: Vec<String> = parse_str_array(&market["outcomes"]);
            let tokens: Vec<String> = parse_str_array(&market["clobTokenIds"]);

            if outcomes.len() != tokens.len() || outcomes.is_empty() {
                continue;
            }

            // push one entry per outcome so the LLM sees both teams
            for (team, token) in outcomes.iter().zip(tokens.iter()) {
                result.push(PolyRawMarket {
                    question: question.clone(),
                    start_time: start_time.clone(),
                    token_id: token.clone(),
                    team: team.clone(),
                });
            }
        }
    }

    Ok(result)
}

fn parse_str_array(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
        serde_json::Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        _ => vec![],
    }
}
async fn ask_llm(
    client: &reqwest::Client,
    sport: &str,
    kalshi: &[KalshiRawMarket],
    poly: &[PolyRawMarket],
) -> anyhow::Result<Vec<MarketPair>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;

    let kalshi_lines: Vec<String> = kalshi
        .iter()
        .map(|m| {
            format!(
                "ticker:{} team:{} event:{}",
                m.ticker, m.yes_sub_title, m.event_ticker
            )
        })
        .collect();

    let poly_lines: Vec<String> = poly
        .iter()
        .map(|m| {
            format!(
                "question:{} team:{} date:{} token:{}",
                m.question, m.team, m.start_time, m.token_id
            )
        })
        .collect();

    let prompt = format!(
        "Match these Kalshi prediction market contracts to Polymarket contracts for the same game and team.\n\
        Same game = same two teams playing on the same date.\n\n\
        Kalshi markets:\n{}\n\nPolymarket markets:\n{}\n\n\
        Return ONLY a JSON array, no explanation. Each element:\n\
        {{\"kalshi_ticker\":\"...\",\"polymarket_token_id\":\"...\",\"team\":\"...\",\"game_date\":\"YYYY-MM-DD\",\"sport\":\"{}\",\"canonical_id\":\"...\"}}\n\
        Only include matches you are confident about.",
        kalshi_lines.join("\n"),
        poly_lines.join("\n"),
        sport
    );

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "gpt-5.4-2026-03-05",
            "messages": [
                {"role": "system", "content": "You are a prediction market matching assistant. Return only valid JSON arrays."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let text = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]");

    // strip markdown code fences if the model wraps output in ```json ... ```
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let pairs: Vec<MarketPair> = serde_json::from_str(cleaned).unwrap_or_else(|e| {
        println!("failed to parse llm response: {}\nraw: {}", e, cleaned);
        vec![]
    });

    Ok(pairs)
}
// sport tuple: (label, kalshi_series_ticker, polymarket_tag_id)
const SPORTS: &[(&str, &str, &str)] =
    &[("MLB", "KXMLBGAME", "100381"), ("WC", "KXWCGAME", "102232")];

pub async fn run_discovery() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut all_pairs: Vec<MarketPair> = Vec::new();

    for (sport, kalshi_series, poly_tag) in SPORTS {
        println!("fetching {} ...", sport);
        let kalshi = fetch_kalshi_raw(&client, kalshi_series).await?;
        let poly = fetch_poly_raw(&client, poly_tag).await?;
        println!(
            "  kalshi: {} markets | poly: {} markets",
            kalshi.len(),
            poly.len()
        );

        if poly.is_empty() {
            println!("  no polymarket markets found, skipping");
            continue;
        }
        let pairs = ask_llm(&client, sport, &kalshi, &poly).await?;
        println!("  matched {} pairs", pairs.len());
        all_pairs.extend(pairs);
    }

    let json = serde_json::to_string_pretty(&all_pairs)?;
    std::fs::write("pairs.json", json)?;
    println!("wrote {} pairs to pairs.json", all_pairs.len());

    Ok(())
}
