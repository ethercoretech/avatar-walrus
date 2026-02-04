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
    
    // === 验证错误（可跳过） ===
    #[error("Invalid gas limit: must be greater than zero")]
    InvalidGas,
    
    #[error("Nonce too low: expected {expected}, got {got}")]
    NonceTooLow { expected: u64, got: u64 },
    
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: String, available: String },
    
    // === 执行错误（严重错误） ===
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

impl ExecutorError {
    /// 判断是否为严重错误（需要回滚整个区块）
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ExecutorError::Database(_) | ExecutorError::Evm(_)
        )
    }
    
    /// 判断是否为验证错误（可以跳过该交易）
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            ExecutorError::InvalidGas
                | ExecutorError::NonceTooLow { .. }
                | ExecutorError::InsufficientFunds { .. }
        )
    }
}
