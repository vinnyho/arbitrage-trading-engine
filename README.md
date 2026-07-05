# Kalshi × Polymarket Arbitrage Engine

Real-time cross-exchange arbitrage detection and execution engine for prediction markets, written in Rust. Watches Kalshi and Polymarket simultaneously over WebSocket, normalizes order books across two structurally different APIs, and detects — and can execute against - price discrepancies for the same real-world event.

> This runs with real money. It's a personal systems-engineering project.

## What it does

- Ingests live WebSocket order book data from Kalshi and Polymarket concurrently
- Normalizes prices into a unified order book despite each exchange modeling markets differently (Kalshi: a single moneyline market per game; Polymarket: sometimes one two-outcome market, sometimes split into separate per-team Yes/No propositions)
- Detects when the same real-world event is mispriced across exchanges
- Places real orders on both exchanges when a profitable spread opens, using exchange-specific signature schemes (RSA-PSS for Kalshi, EIP-712 with a Solady-wrapped deposit-wallet signature for Polymarket)

## Architecture

**`cargo run -- discover`** (offline, run daily)

```
Kalshi REST + Polymarket Gamma API  ──▶  LLM-matched pairs  ──▶  pairs.json
```

**`cargo run`** (live)

```
┌──────────────┐        ┌──────────────────┐
│  kalshi.rs   │        │   polymarket.rs   │
│ (WS feed)    │        │    (WS feed)      │
└──────┬───────┘        └─────────┬────────┘
       │                          │
       ▼                          ▼
       └───────────┬──────────────┘
             shared OrderBook maps
                    │
                    ▼
              scanner.rs
    polls both maps every 100ms, checks
    pairs.json for cross-exchange spreads
                    │
                    ▼  ArbSignal (mpsc channel)
              executor.rs
    places real Kalshi + Polymarket orders,
         logs every attempt to SQLite
                    │
                    ▼
        metrics.rs + server.rs
    live dashboard + /metrics endpoint
```

## Features

- **Dual WebSocket feeds** with automatic reconnect/backoff on both exchanges
- **LLM-assisted market discovery** — matches equivalent contracts across two exchanges with completely different naming and ID schemes
- **Exchange-specific order signing** — Kalshi RSA-PSS REST signing, Polymarket EIP-712 + CTF Exchange V2 deposit-wallet flow
- **Real-money-safe execution semantics** — trade dedup gated on the actual order outcome (not just the attempt), stale order-book detection, and a supervised execution task that fails loud instead of silently going dark
- **Observability** — a live web dashboard, a JSON stats API, and real p50/p99 latency percentiles, not placeholder numbers

## Tech stack

Rust · Tokio · Axum · tokio-tungstenite · SQLite (rusqlite) · reqwest · serde · RSA-PSS & EIP-712 signing (k256, tiny-keccak)

## Dashboard

Running the app starts a live dashboard (default `http://localhost:3000`) showing uptime, message throughput, arb signals detected, and order latency percentiles, refreshed every 2 seconds. Also available as JSON (`/api/stats`).

## Getting started

Requires a Kalshi API key + RSA private key, a Polymarket deposit-wallet setup, and an OpenAI API key for discovery. Copy `.env.example` to `.env` and fill in your credentials.

```bash
# 1. Match today's Kalshi and Polymarket contracts for the same games
cargo run -- discover

# 2. Start the live feeds, scanner, executor, and dashboard
cargo run
```

## Project structure

```
src/
├── main.rs        entry point — discover mode or live scanning mode
├── config.rs      runtime configuration from environment variables
├── discovery.rs   REST + LLM-based market matching → pairs.json
├── kalshi.rs      Kalshi WebSocket feed + RSA-PSS request signing
├── polymarket.rs  Polymarket WebSocket feed
├── poly_sign.rs   Polymarket EIP-712 / deposit-wallet order signing
├── scanner.rs     cross-exchange arbitrage detection loop
├── executor.rs    places real orders on both exchanges
├── metrics.rs     throughput/latency instrumentation
├── server.rs      axum dashboard + /metrics endpoint
└── types.rs       shared data types
```

## Status

Detection, discovery (MLB + World Cup), and both execution paths are live and have been verified against real exchange responses.
