use std::sync::Arc;

use alloy_primitives::bytes;
use eyre::{Result, eyre};
use ethers::{
    providers::{Provider, Ws, StreamExt},
    types::{Filter, H160, H256, U256, BlockId, BlockNumber, Log, Address, H64},
};
use rusqlite::Connection;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::db;
use crate::models::Erc20Transfer;

// keccak256("Transfer(address,address,uint256)")
const TRANSFER_TOPIC: H256 = H256([
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b,
    0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16,
    0x28, 0xf5, 0x5a, 0x4d, 0xf8, 0x3e, 0x34, 0x34
]);

pub async fn run(
    rpc_url: String,
    pol_token: Address,
    binance_addrs: Vec<Address>,
    conn: Connection,
) -> Result<()> {
    let ws = Ws::connect(rpc_url).await?;
    let provider = Provider::new(ws);

    let conn = Arc::new(Mutex::new(conn));
    let binance_set: Vec<H160> = binance_addrs.clone();

    info!("Indexer started. Subscribing to new heads…");

    let mut stream = provider.subscribe_blocks().await?;

    while let Some(header) = stream.next().await {
        let number = header.number.ok_or_else(|| eyre!("no block number"))?.as_u64();
        let hash: H256 = header.hash.unwrap_or_default();
        info!(block = number, ?hash, "New block");

        // Filter logs for this block, POL token, Transfer topic, and (from OR to) in Binance set
        let filter = Filter::new()
            .address(pol_token)
            .topic0(TRANSFER_TOPIC)
            .from_block(number)
            .to_block(number)
            .or_select({
                let mut f = Filter::new().address(pol_token).topic0(TRANSFER_TOPIC).from_block(number).to_block(number);
                f = f.topic1(binance_set.clone()); // from in Binance
                f
            })
            .or_select({
                let mut f = Filter::new().address(pol_token).topic0(TRANSFER_TOPIC).from_block(number).to_block(number);
                f = f.topic2(binance_set.clone()); // to in Binance
                f
            });

        let logs = provider.get_logs(&filter).await?;

        // Fetch timestamp
        let block = provider.get_block(BlockId::Number(BlockNumber::Number(number.into()))).await?;
        let ts_unix = block
            .and_then(|b| b.timestamp.as_u64().into())
            .unwrap_or(0) as i64;

        // Persist block
        {
            let c = conn.lock().await;
            db::insert_block(&c, number, &format!("{:?}", hash), ts_unix)?;
        }

        // Process logs
        let mut delta: i128 = 0; // signed delta on raw units
        for lg in logs {
            if let Some(tr) = decode_transfer(&lg) {
                let from_is_binance = binance_addrs.contains(&tr.from);
                let to_is_binance = binance_addrs.contains(&tr.to);

                // raw value(U256) -> i128 via string (lossless for storage; for math we clamp to i128 range for delta sign, but we also use U256 for accumulation)
                let value_str = tr.value.to_string();

                {
                    let c = conn.lock().await;
                    db::insert_transfer(
                        &c,
                        tr.block_number,
                        &tr.tx_hash,
                        tr.log_index,
                        &format!("{:?}", lg.address),
                        &format!("{:?}", tr.from),
                        &format!("{:?}", tr.to),
                        &value_str,
                        to_is_binance,
                        from_is_binance,
                    )?;
                }

                if to_is_binance && !from_is_binance {
                    // inflow to Binance: +value
                    // For delta sign only; accumulation below uses U256 safe add/sub
                    // Convert to i128 safely by capping at i128::MAX if overflow
                    let part = value_str.parse::<i128>().unwrap_or(i128::MAX);
                    delta = delta.saturating_add(part);
                }
                if from_is_binance && !to_is_binance {
                    let part = value_str.parse::<i128>().unwrap_or(i128::MAX);
                    delta = delta.saturating_sub(part);
                }
            }
        }

        if delta != 0 {
            // Update cumulative using U256 arithmetic for exactness
            let latest = {
                let c = conn.lock().await;
                crate::db::get_latest_cumulative(&c)?
            };
            let mut acc = latest.cumulative_netflow_raw.parse::<U256>().unwrap_or(U256::ZERO);
            if delta > 0 {
                acc = acc.saturating_add(U256::from(delta as u128));
            } else {
                // Avoid underflow: if negative exceeds current acc, clamp to zero
                let sub = U256::from((-delta) as u128);
                if sub > acc { acc = U256::ZERO; }
                else { acc = acc - sub; }
            }
            let acc_str = acc.to_string();
            let c = conn.lock().await;
            db::update_cumulative(&c, number, &acc_str)?;
            info!(block = number, delta = delta, cumulative = %acc_str, "Cumulative updated");
        }
    }

    Ok(())
}

fn decode_transfer(lg: &Log) -> Option<Erc20Transfer> {
    if lg.topics.len() != 3 { return None; }
    if lg.topics[0] != TRANSFER_TOPIC { return None; }

    let from = H160::from_slice(lg.topics[1].as_bytes()[12..].try_into().ok()?);
    let to = H160::from_slice(lg.topics[2].as_bytes()[12..].try_into().ok()?);

    // data is uint256 value (32 bytes)
    let value = if let Some(data) = lg.data.0.get(0..) {
        if data.len() >= 32 {
            U256::from_big_endian(&data[data.len()-32..])
        } else { return None; }
    } else { return None; };

    Some(Erc20Transfer{
        block_number: lg.block_number?.as_u64(),
        tx_hash: format!("{:?}", lg.transaction_hash?),
        log_index: lg.log_index?.as_u64(),
        from,
        to,
        value,
    })
}
