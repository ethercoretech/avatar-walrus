//! 存储根计算
//! 
//! 为单个合约账户计算存储树的根哈希

use alloy_primitives::{Address, U256, B256};
use crate::db::StateDatabase;
use crate::trie::{TrieBuilder, TrieError};
use crate::trie::builder::{hash_key, rlp_encode_storage_value};

/// 存储根计算器
pub struct StorageRootCalculator<'a> {
    db: &'a dyn StateDatabase,
}

impl<'a> StorageRootCalculator<'a> {
    /// 创建存储根计算器
    pub fn new(db: &'a dyn StateDatabase) -> Self {
        Self { db }
    }
    
    /// 计算账户的存储根
    /// 
    /// # 参数
    /// - `address`: 合约地址
    /// 
    /// # 返回
    /// 存储树的根哈希
    pub fn calculate(&self, address: &Address) -> Result<B256, TrieError> {
        // 1. 获取账户的所有存储槽
        let storage_slots = self.db.get_all_storage(address)
            .map_err(|e| TrieError::Database(e.to_string()))?;
        
        // 2. 如果没有存储槽，返回空存储根
        if storage_slots.is_empty() {
            return Ok(EMPTY_STORAGE_ROOT);
        }
        
        // 3. 构建存储树
        let mut builder = TrieBuilder::new();
        
        // 4. 对存储槽按哈希键排序
        let mut sorted_slots: Vec<_> = storage_slots
            .into_iter()
            .filter(|slot| slot.value != U256::ZERO) // 跳过零值（gas 优化）
            .map(|slot| {
                let key_bytes = slot.key.to_be_bytes::<32>();
                let hashed_key = hash_key(&key_bytes);
                (hashed_key, slot.value)
            })
            .collect();
        
        sorted_slots.sort_by_key(|(hash, _)| *hash);
        
        // 5. 插入存储树
        for (hashed_key, value) in sorted_slots {
            let value_rlp = rlp_encode_storage_value(value);
            builder.add_leaf(hashed_key, &value_rlp);
        }
        
        // 6. 计算根哈希
        Ok(builder.root())
    }
    
    /// 增量计算存储根（仅计算变更的槽位）
    /// 
    /// TODO: 实现增量计算优化
    pub fn calculate_incremental(
        &self,
        address: &Address,
        _changed_slots: &[(U256, U256)],
    ) -> Result<B256, TrieError> {
        // 当前简化实现：完整重新计算
        // 生产环境应该利用变更槽位进行增量更新
        self.calculate(address)
    }
}

/// 空存储根（空 MPT 的根哈希）
pub const EMPTY_STORAGE_ROOT: B256 = B256::new([
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RedbStateDB;
    use alloy_primitives::address;
    use tempfile::TempDir;
    
    #[test]
    fn test_empty_storage_root() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        let calculator = StorageRootCalculator::new(&db);
        
        let addr = address!("0000000000000000000000000000000000000001");
        let root = calculator.calculate(&addr).unwrap();
        
        // 空存储应该返回空存储根
        assert_eq!(root, EMPTY_STORAGE_ROOT);
    }
}
