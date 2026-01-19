//! 状态缓存层
//! 
//! 使用 LRU Cache 减少对 Walrus 的读取次数

use alloy_primitives::{Address, U256, B256, Bytes};
use lru::LruCache;
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use crate::schema::Account;

/// 缓存键类型
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum CacheKey {
    Account(Address),
    Storage(Address, U256),
    Code(B256),
    BlockHash(u64),
}

/// 缓存值类型
#[derive(Debug, Clone)]
pub enum CacheValue {
    Account(Account),
    Storage(U256),
    Code(Bytes),
    BlockHash(B256),
}

impl CacheValue {
    pub fn as_account(&self) -> &Account {
        match self {
            CacheValue::Account(acc) => acc,
            _ => panic!("Expected Account, got {:?}", self),
        }
    }
    
    pub fn as_storage(&self) -> U256 {
        match self {
            CacheValue::Storage(val) => *val,
            _ => panic!("Expected Storage, got {:?}", self),
        }
    }
    
    pub fn as_code(&self) -> &Bytes {
        match self {
            CacheValue::Code(code) => code,
            _ => panic!("Expected Code, got {:?}", self),
        }
    }
    
    pub fn as_block_hash(&self) -> B256 {
        match self {
            CacheValue::BlockHash(hash) => *hash,
            _ => panic!("Expected BlockHash, got {:?}", self),
        }
    }
}

/// 状态缓存
/// 
/// 使用 LRU 策略缓存账户、存储、代码、区块哈希
pub struct StateCache {
    cache: RwLock<LruCache<CacheKey, CacheValue>>,
}

impl StateCache {
    /// 创建缓存（默认容量 10000）
    pub fn new() -> Self {
        Self::with_capacity(10000)
    }
    
    /// 创建指定容量的缓存
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap()
            )),
        }
    }
    
    /// 获取缓存值
    pub fn get(&self, key: &CacheKey) -> Option<CacheValue> {
        self.cache.write().get(key).cloned()
    }
    
    /// 设置缓存值
    pub fn put(&self, key: CacheKey, value: CacheValue) {
        self.cache.write().put(key, value);
    }
    
    /// 移除缓存
    pub fn remove(&self, key: &CacheKey) {
        self.cache.write().pop(key);
    }
    
    /// 清空缓存
    pub fn clear(&self) {
        self.cache.write().clear();
    }
    
    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }
    
    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }
}

impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    
    #[test]
    fn test_cache_basic() {
        let cache = StateCache::with_capacity(2);
        let addr = address!("0000000000000000000000000000000000000001");
        let account = Account::default();
        
        // 插入
        cache.put(
            CacheKey::Account(addr),
            CacheValue::Account(account.clone())
        );
        
        // 读取
        let cached = cache.get(&CacheKey::Account(addr)).unwrap();
        assert_eq!(cached.as_account().nonce, account.nonce);
        
        // LRU 淘汰测试
        let addr2 = address!("0000000000000000000000000000000000000002");
        let addr3 = address!("0000000000000000000000000000000000000003");
        
        cache.put(CacheKey::Account(addr2), CacheValue::Account(account.clone()));
        cache.put(CacheKey::Account(addr3), CacheValue::Account(account.clone()));
        
        // addr 应该被淘汰
        assert!(cache.get(&CacheKey::Account(addr)).is_none());
        assert!(cache.get(&CacheKey::Account(addr3)).is_some());
    }
}
