use eyre::{Result, eyre};
use ethers::types::{Address, H160};

#[derive(Debug, Clone)]
pub struct Erc20Transfer {
    pub block_number: u64,
    pub tx_hash: String,
    pub log_index: u64,
    pub from: Address,
    pub to: Address,
    pub value: ethers::types::U256,
}

pub fn parse_address(s: &str) -> Result<Address> {
    s.parse::<H160>()
        .map_err(|_| eyre!("Invalid address: {}", s))
}

pub fn parse_addresses(csv: &str) -> Result<Vec<Address>> {
    let mut out = Vec::new();
    for part in csv.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        out.push(parse_address(part)?);
    }
    if out.is_empty() {
        return Err(eyre!("No Binance addresses provided"));
    }
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct NetflowSnapshot {
    pub block_number: u64,
    pub cumulative_netflow_raw: String, // as U256 string (wei units of token decimals, i.e. raw)
    pub updated_at_unix: i64,
}
