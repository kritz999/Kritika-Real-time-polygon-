use eyre::Result;
use rusqlite::{Connection, params};
use time::OffsetDateTime;

use crate::models::NetflowSnapshot;

pub const SCHEMA_SQL: &str = r#"
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
    value TEXT NOT NULL, -- U256 as decimal string
    is_binance_in BOOLEAN NOT NULL,
    is_binance_out BOOLEAN NOT NULL,
    UNIQUE(tx_hash, log_index)
);

-- Stores the running cumulative netflow value as a raw integer string (no decimals scaling)
CREATE TABLE IF NOT EXISTS cumulative_netflow (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    block_number INTEGER NOT NULL,
    value TEXT NOT NULL, -- U256 decimal string
    updated_at_unix INTEGER NOT NULL
);

-- Bookkeeping
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn init(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch(SCHEMA_SQL)?;

    // Initialize cumulative to zero if missing
    let exists: Option<i64> = conn.query_row(
        "SELECT id FROM cumulative_netflow WHERE id=1",
        [],
        |row| row.get(0)
    ).optional()?;
    if exists.is_none() {
        conn.execute(
            "INSERT INTO cumulative_netflow (id, block_number, value, updated_at_unix) VALUES (1, 0, '0', ?)",
            params![OffsetDateTime::now_utc().unix_timestamp()],
        )?;
    }
    Ok(conn)
}

pub fn insert_block(conn: &Connection, number: u64, hash: &str, ts_unix: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO blocks (block_number, block_hash, ts_unix) VALUES (?, ?, ?)",
        params![number as i64, hash, ts_unix],
    )?;
    Ok(())
}

pub fn insert_transfer(
    conn: &Connection,
    block_number: u64,
    tx_hash: &str,
    log_index: u64,
    token: &str,
    sender: &str,
    recipient: &str,
    value_dec: &str,
    is_binance_in: bool,
    is_binance_out: bool,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO erc20_transfers (block_number, tx_hash, log_index, token, sender, recipient, value, is_binance_in, is_binance_out)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            block_number as i64,
            tx_hash,
            log_index as i64,
            token,
            sender,
            recipient,
            value_dec,
            is_binance_in as i64,
            is_binance_out as i64
        ],
    )?;
    Ok(())
}

pub fn update_cumulative(conn: &Connection, block_number: u64, new_value_dec: &str) -> Result<()> {
    conn.execute(
        "UPDATE cumulative_netflow SET block_number=?, value=?, updated_at_unix=? WHERE id=1",
        params![block_number as i64, new_value_dec, OffsetDateTime::now_utc().unix_timestamp()],
    )?;
    Ok(())
}

pub fn get_latest_cumulative(conn: &Connection) -> Result<NetflowSnapshot> {
    let mut stmt = conn.prepare("SELECT block_number, value, updated_at_unix FROM cumulative_netflow WHERE id=1")?;
    let row = stmt.query_row([], |row| {
        Ok(NetflowSnapshot{
            block_number: row.get::<_, i64>(0)? as u64,
            cumulative_netflow_raw: row.get::<_, String>(1)?,
            updated_at_unix: row.get::<_, i64>(2)?,
        })
    })?;
    Ok(row)
}
