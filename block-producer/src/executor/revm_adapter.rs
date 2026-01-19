//! REVM Database Trait 适配器
//! 
//! 将 WalrusStateDB 适配为 REVM 的 Database trait

use alloy_primitives::{Address, U256, B256, Bytes};
use revm::primitives::{AccountInfo, Bytecode};
use revm::Database;
use crate::db::{StateDatabase, WalrusStateDB, DbError};
use crate::schema::account::EMPTY_CODE_HASH;

/// REVM 数据库适配器
/// 
/// 包装 WalrusStateDB，实现 REVM 的 Database trait
pub struct RevmAdapter {
    db: WalrusStateDB,
}

impl RevmAdapter {
    /// 创建新的适配器
    pub fn new(db: WalrusStateDB) -> Self {
        Self { db }
    }
    
    /// 获取内部数据库的可变引用
    pub fn db_mut(&mut self) -> &mut WalrusStateDB {
        &mut self.db
    }
    
    /// 获取内部数据库的引用
    pub fn db(&self) -> &WalrusStateDB {
        &self.db
    }
}

impl Database for RevmAdapter {
    type Error = DbError;
    
    /// 获取账户基本信息
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let account = self.db.get_account(&address)?;
        
        Ok(account.map(|acc| AccountInfo {
            balance: acc.balance,
            nonce: acc.nonce,
            code_hash: acc.code_hash,
            code: None, // 延迟加载代码
        }))
    }
    
    /// 根据哈希获取合约字节码
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // 空代码哈希直接返回空字节码
        if code_hash == EMPTY_CODE_HASH {
            return Ok(Bytecode::new());
        }
        
        let code = self.db.get_code(&code_hash)?
            .ok_or_else(|| DbError::CodeNotFound(code_hash))?;
        
        // 将字节码转换为 REVM 的 Bytecode 类型
        Ok(Bytecode::new_raw(code))
    }
    
    /// 获取存储槽值
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.db.get_storage(&address, index)
    }
    
    /// 获取区块哈希
    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.db.get_block_hash(number)?
            .ok_or_else(|| DbError::BlockNotFound(number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Account;
    use alloy_primitives::address;
    
    #[test]
    fn test_revm_adapter_basic() {
        let mut db = WalrusStateDB::new().unwrap();
        let addr = address!("0000000000000000000000000000000000000001");
        
        // 设置账户
        let account = Account::with_balance(U256::from(1000));
        db.set_account(&addr, account).unwrap();
        
        // 通过适配器读取
        let mut adapter = RevmAdapter::new(db);
        let info = adapter.basic(addr).unwrap().unwrap();
        
        assert_eq!(info.balance, U256::from(1000));
        assert_eq!(info.nonce, 0);
    }
    
    #[test]
    fn test_revm_adapter_storage() {
        let mut db = WalrusStateDB::new().unwrap();
        let addr = address!("0000000000000000000000000000000000000001");
        
        // 设置存储
        db.set_storage(&addr, U256::from(1), U256::from(100)).unwrap();
        
        // 通过适配器读取
        let mut adapter = RevmAdapter::new(db);
        let value = adapter.storage(addr, U256::from(1)).unwrap();
        
        assert_eq!(value, U256::from(100));
    }
}
