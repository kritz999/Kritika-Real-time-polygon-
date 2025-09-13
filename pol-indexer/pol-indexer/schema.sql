PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS blocks (
    block_number INTEGER PRIMARY KEY,
    block_hash TEXT NOT NULL,
    ts_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS erc20_transfers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_number INTEGER NOT NULL,
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    token TEXT NOT NULL,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    value TEXT NOT NULL,
    is_binance_in BOOLEAN NOT NULL,
    is_binance_out BOOLEAN NOT NULL,
    UNIQUE(tx_hash, log_index)
);
CREATE TABLE IF NOT EXISTS cumulative_netflow (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    block_number INTEGER NOT NULL,
    value TEXT NOT NULL,
    updated_at_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
