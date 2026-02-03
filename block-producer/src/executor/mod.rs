//! EVM 执行器
//! 
//! 基于 REVM 实现交易和区块执行

pub mod revm_adapter;
pub mod transaction;
pub mod block_executor;
pub mod receipts;

pub use revm_adapter::RevmAdapter;
pub use transaction::{TransactionExecutor, ExecutionResult};
pub use block_executor::{BlockExecutor, BlockExecutionResult};
pub use receipts::ReceiptBuilder;

use thiserror::Error;

/// 执行器错误类型
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("EVM error: {0}")]
    Evm(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Gas limit exceeded")]
    GasLimitExceeded,
    
    #[error("Insufficient balance")]
    InsufficientBalance,
    
    #[error("Invalid nonce")]
    InvalidNonce,
    
    #[error("Other error: {0}")]
    Other(String),
}

// 为 ExecutorError 实现 From<String>，支持 ? 操作符
impl From<String> for ExecutorError {
    fn from(s: String) -> Self {
        ExecutorError::Other(s)
    }
}
