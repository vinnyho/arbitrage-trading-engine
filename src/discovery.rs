use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct MarketPair {
    pub canonical_id: String,
    pub sport: String,
    pub game_date: String,
    pub team: String,
    pub kalshi_ticker: String,
    pub polymarket_token_id: String,
}

fn mlb_team_table() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("AZ", "Arizona Diamondbacks"),
        ("ATL", "Atlanta Braves"),
        ("BAL", "Baltimore Orioles"),
        ("BOS", "Boston Red Sox"),
        ("CHC", "Chicago Cubs"),
        ("CWS", "Chicago White Sox"),
        ("CIN", "Cincinnati Reds"),
        ("CLE", "Cleveland Guardians"),
        ("COL", "Colorado Rockies"),
        ("DET", "Detroit Tigers"),
        ("HOU", "Houston Astros"),
        ("KC", "Kansas City Royals"),
        ("LAA", "Los Angeles Angels"),
        ("LAD", "Los Angeles Dodgers"),
        ("MIA", "Miami Marlins"),
        ("MIL", "Milwaukee Brewers"),
        ("MIN", "Minnesota Twins"),
        ("NYM", "New York Mets"),
        ("NYY", "New York Yankees"),
        ("ATH", "Athletics"),
        ("PHI", "Philadelphia Phillies"),
        ("PIT", "Pittsburgh Pirates"),
        ("SD", "San Diego Padres"),
        ("SF", "San Francisco Giants"),
        ("SEA", "Seattle Mariners"),
        ("STL", "St. Louis Cardinals"),
        ("TB", "Tampa Bay Rays"),
        ("TEX", "Texas Rangers"),
        ("TOR", "Toronto Blue Jays"),
        ("WSH", "Washington Nationals"),
    ])
}

fn month_num(mon: &str) -> Option<u32> {
    Some(match mon {
        "JAN" => 1,
        "FEB" => 2,
        "MAR" => 3,
        "APR" => 4,
        "MAY" => 5,
        "JUN" => 6,
        "JUL" => 7,
        "AUG" => 8,
        "SEP" => 9,
        "OCT" => 10,
        "NOV" => 11,
        "DEC" => 12,
        _ => return None,
    })
}

fn parse_kalshi_ticker(ticker: &str) -> Option<(String, String)> {
    let rest = ticker.strip_prefix("KXMLBGAME-")?;
    let team_code = rest.rsplit('-').next()?.to_string();
    let date_part = rest.get(0..7)?;
    let yy: i32 = date_part.get(0..2)?.parse().ok()?;
    let mon = month_num(date_part.get(2..5)?)?;
    let dd: u32 = date_part.get(5..7)?.parse().ok()?;
    let date = format!("{:04}-{:02}-{:02}", 2000 + yy, mon, dd);
    Some((date, team_code))
}

// ---------- Kalshi REST ----------

#[derive(Debug, Deserialize)]
struct KalshiMarketsResp {
    markets: Vec<KalshiMarket>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KalshiMarket {
    ticker: String,
    event_ticker: String,
}

struct KalshiGame {
    date: String,
    teams: HashMap<String, String>,
}

async fn fetch_kalshi_games(
    client: &reqwest::Client,
) -> anyhow::Result<HashMap<String, KalshiGame>> {
    let table = mlb_team_table();
    let mut games: HashMap<String, KalshiGame> = HashMap::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut req = client
            .get("https://external-api.kalshi.com/trade-api/v2/markets")
            .query(&[
                ("series_ticker", "KXMLBGAME"),
                ("status", "open"),
                ("limit", "1000"),
            ]);
        if let Some(c) = &cursor {
            req = req.query(&[("cursor", c.as_str())]);
        }

        let resp: KalshiMarketsResp = req.send().await?.json().await?;

        for m in &resp.markets {
            let Some((date, code)) = parse_kalshi_ticker(&m.ticker) else {
                continue;
            };
            let Some(full_name) = table.get(code.as_str()) else {
                continue;
            };
            games
                .entry(m.event_ticker.clone())
                .or_insert_with(|| KalshiGame {
                    date: date.clone(),
                    teams: HashMap::new(),
                })
                .teams
                .insert(full_name.to_string(), m.ticker.clone());
        }

        match resp.cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }

    Ok(games)
}

// ---------- Polymarket REST ----------

async fn fetch_poly_moneyline(
    client: &reqwest::Client,
    team_a: &str,
    team_b: &str,
    kalshi_date: &str,
) -> anyhow::Result<Option<HashMap<String, String>>> {
    let q = format!("{} {}", team_a, team_b);
    let resp: Value = client
        .get("https://gamma-api.polymarket.com/public-search")
        .query(&[
            ("q", q.as_str()),
            ("limit_per_type", "20"),
            ("events_status", "active"),
        ])
        .send()
        .await?
        .json()
        .await?;

    let Some(events) = resp.get("events").and_then(|e| e.as_array()) else {
        return Ok(None);
    };

    for event in events {
        let Some(markets) = event.get("markets").and_then(|m| m.as_array()) else {
            continue;
        };

        let Some(ml) = markets
            .iter()
            .find(|m| m.get("sportsMarketType").and_then(|v| v.as_str()) == Some("moneyline"))
        else {
            continue;
        };

        let outcomes = parse_str_array(ml.get("outcomes"));
        let tokens = parse_str_array(ml.get("clobTokenIds"));
        if outcomes.len() != 2 || tokens.len() != 2 {
            continue;
        }

        let has_both = outcomes.iter().any(|o| o == team_a) && outcomes.iter().any(|o| o == team_b);
        if !has_both {
            continue;
        }

        let poly_date = event
            .get("startTime")
            .and_then(|v| v.as_str())
            .and_then(to_et_date);
        if poly_date.as_deref() != Some(kalshi_date) {
            continue;
        }

        let mut map = HashMap::new();
        for (outcome, token) in outcomes.iter().zip(tokens.iter()) {
            map.insert(outcome.clone(), token.clone());
        }
        return Ok(Some(map));
    }

    Ok(None)
}

// Polymarket encodes arrays as strings like: "[\"Yes\", \"No\"]"
fn parse_str_array(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_default(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn to_et_date(iso: &str) -> Option<String> {
    let dt: DateTime<Utc> = iso.parse().ok()?;
    // Polymarket startTime is UTC; shift to Eastern (EDT) so late games match Kalshi's date
    let et = dt - Duration::hours(4);
    Some(et.format("%Y-%m-%d").to_string())
}

// ---------- Orchestration ----------

pub async fn run_discovery() -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    println!("fetching Kalshi MLB games...");
    let games = fetch_kalshi_games(&client).await?;
    println!("found {} Kalshi games", games.len());

    let mut pairs: Vec<MarketPair> = Vec::new();

    for game in games.values() {
        if game.teams.len() != 2 {
            continue;
        }
        let team_names: Vec<&String> = game.teams.keys().collect();
        let (a, b) = (team_names[0], team_names[1]);

        let poly = match fetch_poly_moneyline(&client, a, b, &game.date).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                println!("  no Polymarket match for {} vs {} on {}", a, b, game.date);
                continue;
            }
            Err(e) => {
                println!("  poly lookup error for {} vs {}: {}", a, b, e);
                continue;
            }
        };

        for (team, kalshi_ticker) in &game.teams {
            let Some(token) = poly.get(team) else {
                continue;
            };
            let canonical_id = format!("MLB-{}-{}", game.date, team.replace(' ', "_"));
            println!(
                "  MATCH {} | kalshi {} <-> poly {}",
                canonical_id, kalshi_ticker, token
            );
            pairs.push(MarketPair {
                canonical_id,
                sport: "MLB".to_string(),
                game_date: game.date.clone(),
                team: team.clone(),
                kalshi_ticker: kalshi_ticker.clone(),
                polymarket_token_id: token.clone(),
            });
        }
    }

    let json = serde_json::to_string_pretty(&pairs)?;
    std::fs::write("pairs.json", json)?;
    println!("\nwrote {} pairs to pairs.json", pairs.len());

    Ok(())
}
