# Real-time Polygon POL Net-Flow Indexer (→ Binance)

This project is a **real-time indexer** that watches the **Polygon** blockchain for **POL** token transfers and computes **cumulative net-flows to Binance** (inflows to Binance minus outflows from Binance), from the moment you start it. **No historical backfill**.

> ⚠️ **You must provide the POL token contract address on Polygon** and a list of Binance addresses (given below) via environment variables or CLI flags.

---

## Features

- **Real-time**: Subscribes to new blocks via WebSocket RPC.
- **Accurate filtering**: Looks for `Transfer(address,address,uint256)` logs on the POL token contract and filters where **from** or **to** is a Binance address.
- **SQLite storage**:
  - Raw blocks
  - Raw ERC-20 transfers (POL only)
  - Running **cumulative net-flow** value (raw token units, as a decimal string)
- **Simple query interfaces**:
  - CLI: `pol-indexer query`
  - HTTP: `GET /netflow` returns JSON
- **Scalable design**:
  - Binance addresses are configurable.
  - Extend to multiple exchanges by adding address lists and additional cumulative tracks.
  - Stateless indexer logic w/ idempotent inserts; safe to restart.

---

## Quick Start

### 1) Requirements

- Rust (stable), Cargo
- A Polygon **WebSocket** RPC endpoint (e.g., from your infra provider)
- The **POL token contract address on Polygon** (set via env or CLI)
- The provided Binance addresses (below)

### 2) Configure

Create a `.env` file (or pass CLI flags):

```env
# .env
RPC_URL=wss://your-polygon-ws-endpoint
DB_PATH=pol_indexer.sqlite

# REQUIRED: POL token contract address on Polygon
POL_TOKEN_ADDRESS=0xYOUR_POL_CONTRACT_ON_POLYGON

# Provided Binance labels (comma-separated, case-insensitive)
BINANCE_ADDRESSES=0xF977814e90dA44bFA03b6295A0616a897441aceC,0xe7804c37c13166fF0b37F5aE0BB07A3aEbb6e245,0x505e71695E9bc45943c58adEC1650577BcA68fD9,0x290275e3db66394C52272398959845170E4DCb88,0xD5C08681719445A5Fdce2Bda98b341A49050d821,0x082489A616aB4D46d1947eE3F912e080815b08DA

# Optional: HTTP API bind
HTTP_BIND=127.0.0.1:8080
```

> **Note**: The project does not backfill. It starts from the first block it sees after launch.

### 3) Build & Run

```bash
cargo build --release
# Run indexer + API server (if HTTP_BIND is set)
./target/release/pol-indexer run

# Or show current cumulative (raw units) at any time:
./target/release/pol-indexer query
```

### 4) HTTP API

```
GET /netflow  -> 200 OK
{
  "block_number": 12345678,
  "cumulative_netflow_raw": "123450000000000000000",
  "updated_at_unix": 1725600000
}
```

- `cumulative_netflow_raw` is a **decimal string of raw token units** (i.e., not adjusted for decimals). If the POL token has 18 decimals, divide by `1e18` for human-readable POL.

---

## Database Schema

See `db::SCHEMA_SQL` or run:

```bash
./target/release/pol-indexer schema
```

Key tables:

- `blocks(block_number, block_hash, ts_unix)`
- `erc20_transfers(block_number, tx_hash, log_index, token, sender, recipient, value, is_binance_in, is_binance_out)`
- `cumulative_netflow(id=1, block_number, value, updated_at_unix)`

---

## How It Works (Data Flow)

1. **Subscribe to new block headers** via WebSocket.
2. For each block `N`, request **logs** filtered by:
   - `address = POL_TOKEN_ADDRESS`
   - `topic0 = keccak("Transfer(address,address,uint256)")`
   - `topic1 = any(BINANCE_ADDRESSES)` **OR** `topic2 = any(BINANCE_ADDRESSES)`
   - `fromBlock = toBlock = N`
3. Decode each `Transfer`:
   - Insert into `erc20_transfers` (idempotent on `(tx_hash, log_index)`).
   - Compute **delta**:
     - `+value` for transfers **to** Binance (inflow)
     - `-value` for transfers **from** Binance (outflow)
     - Ignore internal Binance-to-Binance moves (net 0)
4. Update the **running cumulative** (`cumulative_netflow.value`) using **`U256`** safe arithmetic and clamp to zero on underflow.

---

## Scalability Strategy

- **Multiple exchanges**: maintain multiple cumulative rows (e.g., keyed by `exchange`), or a separate table `cumulative_netflow(exchange TEXT PRIMARY KEY, ...)`. Add address sets per exchange.
- **Multiple tokens**: generalize filter to a list of token contracts; persist `token` column per transfer.
- **High throughput**:
  - Batch `getLogs` over ranges if you miss blocks.
  - Use **block subscription + log subscription** for lower latency.
  - Offload database writes to a bounded channel + writer task.
  - Consider upgrading to **PostgreSQL** for concurrent writes and analytics.
- **Fault tolerance**:
  - Keep last processed block in `state` and, on restart, optionally **fast-forward** through missed blocks (still "near-real-time"; backfill beyond restart window is out of scope for this phase).
- **Extensibility**:
  - Extract an `Exchange` abstraction: a name + set of addresses.
  - Expose Prometheus metrics for health and lag monitoring.

---

## Security & Correctness Notes

- Use **trusted Polygon RPC**. In production, run multiple providers and cross-check.
- The provided addresses are treated as **Polygon addresses**; ensure that they are relevant for Polygon (some Binance labels are multi-chain).
- The indexer computes using **raw token units**; consumers can scale by token decimals when needed.
- All inserts are **idempotent**; duplicates are ignored.

---

## Development Tips

- Enable verbose logs:
  ```bash
  RUST_LOG=pol-indexer=debug,info ./target/release/pol-indexer run
  ```
- Test locally with an ephemeral DB:
  ```bash
  DB_PATH=:memory: ./target/release/pol-indexer run
  ```

---

## Provided Binance Addresses

```
0xF977814e90dA44bFA03b6295A0616a897441aceC
0xe7804c37c13166fF0b37F5aE0BB07A3aEbb6e245
0x505e71695E9bc45943c58adEC1650577BcA68fD9
0x290275e3db66394C52272398959845170E4DCb88
0xD5C08681719445A5Fdce2Bda98b341A49050d821
0x082489A616aB4D46d1947eE3F912e080815b08DA
```

---

## License

MIT
