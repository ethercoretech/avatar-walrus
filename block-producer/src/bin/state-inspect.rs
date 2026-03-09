use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use block_producer::db::RedbStateDB;
use block_producer::db::StateDatabase;
use alloy_primitives::{Address, U256};
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(author, version, about = "Inspect block-producer redb state DB (readonly)")]
struct Args {
    /// Path to redb state database file
    #[arg(long, default_value = "./data/block_producer_state_blockchain-txs.redb")]
    db_path: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List blocks and verify parent_hash linkage.
    Blocks {
        /// First block number (inclusive)
        #[arg(long, default_value_t = 0)]
        from: u64,

        /// Last block number (inclusive). If omitted, stops at first gap.
        #[arg(long)]
        to: Option<u64>,

        /// Stop after this many printed blocks (safety cap).
        #[arg(long, default_value_t = 200)]
        limit: u64,
    },

    /// Dump storage slots for an address (and optionally a single key).
    Storage {
        /// 20-byte hex address (0x...)
        #[arg(long)]
        address: String,

        /// Optional slot key (U256) in hex (0x...) or decimal
        #[arg(long)]
        key: Option<String>,
    },
}

fn parse_address(s: &str) -> Result<Address> {
    let hex = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    let bytes = hex::decode(hex).map_err(|e| anyhow!("invalid address hex: {e}"))?;
    if bytes.len() != 20 {
        return Err(anyhow!("address must be 20 bytes, got {}", bytes.len()));
    }
    Ok(Address::from_slice(&bytes))
}

fn parse_u256(s: &str) -> Result<U256> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        let bytes = hex::decode(hex).map_err(|e| anyhow!("invalid U256 hex: {e}"))?;
        if bytes.len() > 32 {
            return Err(anyhow!("U256 hex too long: {} bytes", bytes.len()));
        }
        let mut padded = [0u8; 32];
        padded[32 - bytes.len()..].copy_from_slice(&bytes);
        return Ok(U256::from_be_bytes(padded));
    }
    Ok(U256::from_str(t).map_err(|e| anyhow!("invalid U256 decimal: {e}"))?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let db = RedbStateDB::open_readonly(&args.db_path)?;

    match args.command {
        Command::Blocks { from, to, limit } => {
            let mut printed = 0u64;
            let mut prev_hash: Option<String> = None;
            let mut prev_number: Option<u64> = None;

            let mut n = from;
            loop {
                if printed >= limit {
                    break;
                }
                if let Some(end) = to {
                    if n > end {
                        break;
                    }
                }

                let Some(block) = db.get_block(n)? else {
                    break;
                };

                let h = block.hash();
                let parent = block.header.parent_hash.clone();

                let mut link_ok = None;
                if let (Some(ph), Some(pn)) = (&prev_hash, prev_number) {
                    if pn + 1 == n {
                        link_ok = Some(parent == *ph);
                    }
                }

                match link_ok {
                    Some(true) => {
                        println!(
                            "#{} hash={} parent={} link=OK",
                            block.header.number, h, parent
                        );
                    }
                    Some(false) => {
                        println!(
                            "#{} hash={} parent={} link=BAD(expected_parent={})",
                            block.header.number, h, parent, prev_hash.unwrap_or_default()
                        );
                    }
                    None => {
                        println!("#{} hash={} parent={}", block.header.number, h, parent);
                    }
                }

                prev_hash = Some(h);
                prev_number = Some(block.header.number);
                printed += 1;
                n += 1;
            }

            if printed == 0 {
                println!("NO_BLOCKS");
            }
        }

        Command::Storage { address, key } => {
            let addr = parse_address(&address)?;
            let mut slots = db.get_all_storage(&addr)?;
            slots.sort_by_key(|s| s.key);

            if let Some(k) = key {
                let k = parse_u256(&k)?;
                let v = db.get_storage(&addr, k)?;
                println!("address=0x{} key=0x{:x} value=0x{:x}", hex::encode(addr), k, v);
                return Ok(());
            }

            println!("address=0x{} slots={}", hex::encode(addr), slots.len());
            for s in slots {
                if s.value != U256::ZERO {
                    println!("  key=0x{:x} value=0x{:x}", s.key, s.value);
                }
            }
        }
    }

    Ok(())
}

