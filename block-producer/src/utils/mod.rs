//! 工具模块

pub mod serialization;
pub mod hash;

pub use serialization::{serialize_to_bytes, deserialize_from_bytes};
pub use hash::{keccak256_hash, compute_tx_hash, compute_block_hash};
