//! Database Trait 定义
//! 
//! 定义状态数据库的核心接口，支持账户、存储、代码的 CRUD 操作

use alloy_primitives::{Address, U256, B256, Bytes};
use crate::schema::{Account, StorageSlot};
use std::collections::HashMap;
use thiserror::Error;

/// 数据库错误类型
#[derive(Debug, Error)]
pub enum DbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Account not found: {0}")]
    AccountNotFound(Address),
    
    #[error("Code not found: {0}")]
    CodeNotFound(B256),
    
    #[error("Block not found: {0}")]
    BlockNotFound(u64),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Walrus error: {0}")]
    Walrus(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// 状态数据库接口
/// 
/// 提供账户、存储、代码的读写操作，支持事务
pub trait StateDatabase: Send + Sync {
    // ==================== 账户操作 ====================
    
    /// 获取账户信息
    fn get_account(&self, address: &Address) -> Result<Option<Account>, DbError>;
    
    /// 设置账户信息
    fn set_account(&mut self, address: &Address, account: Account) -> Result<(), DbError>;
    
    /// 删除账户
    fn delete_account(&mut self, address: &Address) -> Result<(), DbError>;
    
    /// 批量获取账户（性能优化）
    fn batch_get_accounts(&self, addresses: &[Address]) -> Result<Vec<Option<Account>>, DbError> {
        addresses.iter()
            .map(|addr| self.get_account(addr))
            .collect()
    }
    
    // ==================== 存储操作 ====================
    
    /// 获取存储槽值
    fn get_storage(&self, address: &Address, key: U256) -> Result<U256, DbError>;
    
    /// 设置存储槽值
    fn set_storage(&mut self, address: &Address, key: U256, value: U256) -> Result<(), DbError>;
    
    /// 获取账户的所有存储槽（用于计算存储根）
    fn get_all_storage(&self, address: &Address) -> Result<Vec<StorageSlot>, DbError>;
    
    /// 批量设置存储槽（性能优化）
    fn batch_set_storage(&mut self, changes: &[(Address, U256, U256)]) -> Result<(), DbError> {
        for (address, key, value) in changes {
            self.set_storage(address, *key, *value)?;
        }
        Ok(())
    }
    
    // ==================== 代码操作 ====================
    
    /// 获取合约字节码
    fn get_code(&self, code_hash: &B256) -> Result<Option<Bytes>, DbError>;
    
    /// 设置合约字节码
    fn set_code(&mut self, code_hash: B256, code: Bytes) -> Result<(), DbError>;
    
    // ==================== 区块哈希 ====================
    
    /// 获取区块哈希
    fn get_block_hash(&self, block_number: u64) -> Result<Option<B256>, DbError>;
    
    /// 设置区块哈希
    fn set_block_hash(&mut self, block_number: u64, block_hash: B256) -> Result<(), DbError>;
    
    // ==================== 事务支持 ====================
    
    /// 开始事务
    fn begin_transaction(&mut self) -> Result<(), DbError>;
    
    /// 提交事务
    fn commit_transaction(&mut self) -> Result<(), DbError>;
    
    /// 回滚事务
    fn rollback_transaction(&mut self) -> Result<(), DbError>;
    
    // ==================== 辅助方法 ====================
    
    /// 获取所有变更的账户（用于增量状态根计算）
    fn get_changed_accounts(&self) -> Result<Vec<Address>, DbError> {
        // 默认实现返回空，子类可以重写
        Ok(Vec::new())
    }
    
    /// 清除缓存
    fn clear_cache(&mut self) -> Result<(), DbError> {
        Ok(())
    }
}

/// 事务缓冲区（用于支持回滚）
#[derive(Debug, Clone, Default)]
pub struct TransactionBuffer {
    pub accounts: HashMap<Address, Account>,
    pub storage: HashMap<(Address, U256), U256>,
    pub codes: HashMap<B256, Bytes>,
    pub block_hashes: HashMap<u64, B256>,
    pub deleted_accounts: Vec<Address>,
}

impl TransactionBuffer {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn clear(&mut self) {
        self.accounts.clear();
        self.storage.clear();
        self.codes.clear();
        self.block_hashes.clear();
        self.deleted_accounts.clear();
    }
    
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty() 
            && self.storage.is_empty() 
            && self.codes.is_empty()
            && self.block_hashes.is_empty()
            && self.deleted_accounts.is_empty()
    }
}
