//! Merkle Patricia Trie 实现
//! 
//! 用于计算状态根和存储根

pub mod builder;
pub mod state_root;
pub mod storage_root;
pub mod proof;

pub use builder::TrieBuilder;
pub use state_root::StateRootCalculator;
pub use storage_root::StorageRootCalculator;
pub use proof::{MerkleProof, ProofVerifier};

use thiserror::Error;

/// Trie 错误类型
#[derive(Debug, Error)]
pub enum TrieError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Invalid proof")]
    InvalidProof,
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("RLP encoding error: {0}")]
    RlpEncoding(String),
    
    #[error("Other error: {0}")]
    Other(String),
}
