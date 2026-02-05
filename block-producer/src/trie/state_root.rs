//! 状态根计算
//! 
//! 计算全局状态树的根哈希

use alloy_primitives::{Address, B256};
use rayon::prelude::*;
use crate::db::StateDatabase;
use crate::trie::{TrieBuilder, TrieError, StorageRootCalculator};
use crate::trie::builder::{hash_key, rlp_encode_account};

/// 状态根计算器
pub struct StateRootCalculator<'a> {
    db: &'a dyn StateDatabase,
    /// 是否使用并行计算
    parallel: bool,
}

impl<'a> StateRootCalculator<'a> {
    /// 创建状态根计算器
    pub fn new(db: &'a dyn StateDatabase) -> Self {
        Self {
            db,
            parallel: true,
        }
    }
    
    /// 创建串行计算器（用于调试）
    pub fn new_serial(db: &'a dyn StateDatabase) -> Self {
        Self {
            db,
            parallel: false,
        }
    }
    
    /// 计算状态根（全量计算）
    /// 
    /// 遍历所有账户，计算状态树根哈希
    pub fn calculate(&self) -> Result<B256, TrieError> {
        // TODO: 实现完整的账户遍历
        // 当前简化实现：仅计算变更账户
        self.calculate_incremental()
    }
    
    /// 增量计算状态根（仅计算变更账户）
    /// 
    /// 性能优化：只重新计算变更的账户及其路径
    pub fn calculate_incremental(&self) -> Result<B256, TrieError> {
        // 1. 获取变更的账户列表
        let changed_accounts = self.db.get_changed_accounts()
            .map_err(|e| TrieError::Database(e.to_string()))?;
        
        if changed_accounts.is_empty() {
            // 没有变更，返回空状态根
            return Ok(EMPTY_STATE_ROOT);
        }
        
        // 2. 为每个账户计算存储根（并行）
        let accounts_with_storage: Vec<_> = if self.parallel {
            changed_accounts
                .par_iter()
                .map(|addr| self.process_account(addr))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            changed_accounts
                .iter()
                .map(|addr| self.process_account(addr))
                .collect::<Result<Vec<_>, _>>()?
        };
        
        // 3. 构建状态树
        let mut builder = TrieBuilder::new();
        
        // 4. 按地址哈希排序
        let mut sorted_accounts = accounts_with_storage;
        sorted_accounts.sort_by_key(|(hashed_addr, _, _, _, _)| *hashed_addr);
        
        // 5. 插入状态树
        for (hashed_addr, nonce, balance, storage_root, code_hash) in sorted_accounts {
            let account_rlp = rlp_encode_account(nonce, balance, storage_root, code_hash);
            builder.add_leaf(hashed_addr, &account_rlp);
        }
        
        // 6. 计算根哈希
        Ok(builder.root())
    }
    
    /// 处理单个账户
    /// 
    /// 返回：(哈希地址, nonce, balance, storage_root, code_hash)
    fn process_account(
        &self,
        address: &Address,
    ) -> Result<(B256, u64, alloy_primitives::U256, B256, B256), TrieError> {
        // 1. 获取账户信息
        let account = self.db.get_account(address)
            .map_err(|e| TrieError::Database(e.to_string()))?
            .ok_or_else(|| TrieError::Database(format!("Account not found: {}", address)))?;
        
        // 2. 计算存储根
        let storage_calculator = StorageRootCalculator::new(self.db);
        let storage_root = storage_calculator.calculate(address)?;
        
        // 3. 哈希地址
        let hashed_addr = hash_key(address.as_slice());
        
        Ok((
            hashed_addr,
            account.nonce,
            account.balance,
            storage_root,
            account.code_hash,
        ))
    }
}

/// 空状态根（空 MPT 的根哈希）
pub const EMPTY_STATE_ROOT: B256 = B256::new([
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RedbStateDB;
    use tempfile::TempDir;
    
    #[test]
    fn test_empty_state_root() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        let calculator = StateRootCalculator::new(&db);
        
        let root = calculator.calculate_incremental().unwrap();
        
        // 空状态应该返回空状态根
        assert_eq!(root, EMPTY_STATE_ROOT);
    }
}
