//! 键值状态数据库实现
//! 
//! 使用 Walrus 作为交易排序器，提供按序写入和读取能力
//! Walrus 本身不作为持久化存储，而是用于确保状态变更的顺序性

use alloy_primitives::{Address, U256, B256, Bytes};
use parking_lot::RwLock;
use std::sync::Arc;
use walrus_rust::{Walrus, Entry};
use crate::db::{StateDatabase, DbError, TransactionBuffer};
use crate::db::cache::{StateCache, CacheKey, CacheValue};
use crate::schema::{Account, StorageSlot};

/// 键值状态数据库
/// 
/// 使用 Walrus 作为交易排序器，配合缓存和事务支持实现状态管理
pub struct WalrusStateDB {
    /// Walrus 实例
    wal: Arc<Walrus>,
    /// LRU 缓存
    cache: Arc<StateCache>,
    /// 事务缓冲区
    tx_buffer: RwLock<Option<TransactionBuffer>>,
    /// 变更追踪（用于增量状态根计算）
    changed_accounts: RwLock<Vec<Address>>,
}

impl WalrusStateDB {
    /// 创建新的 Walrus 状态数据库
    pub fn new() -> Result<Self, DbError> {
        let wal = Arc::new(
            Walrus::new_for_key("evm_state")
                .map_err(|e| DbError::Walrus(e.to_string()))?
        );
        
        Ok(Self {
            wal,
            cache: Arc::new(StateCache::new()),
            tx_buffer: RwLock::new(None),
            changed_accounts: RwLock::new(Vec::new()),
        })
    }
    
    /// 序列化账户并写入 Walrus
    fn persist_account(&self, address: &Address, account: &Account) -> Result<(), DbError> {
        let topic = format!("accounts:{}", hex::encode(address));
        let data = bincode::serialize(account)
            .map_err(|e| DbError::Serialization(e.to_string()))?;
        
        self.wal.append_for_topic(&topic, &data)
            .map_err(|e| DbError::Walrus(e.to_string()))?;
        
        Ok(())
    }
    
    /// 从 Walrus 读取账户（最新版本）
    fn load_account(&self, address: &Address) -> Result<Option<Account>, DbError> {
        // 1. 尝试从缓存读取
        if let Some(cached) = self.cache.get(&CacheKey::Account(*address)) {
            return Ok(Some(cached.as_account().clone()));
        }
        
        // 2. 从 Walrus 读取最新条目
        let topic = format!("accounts:{}", hex::encode(address));
        
        // TODO: 优化 - 维护索引快速定位最新条目
        // 当前实现：读取整个 topic 的最后一条记录
        if let Some(entry) = self.read_latest_for_topic(&topic)? {
            let account: Account = bincode::deserialize(&entry.data)
                .map_err(|e| DbError::Serialization(e.to_string()))?;
            
            // 3. 更新缓存
            self.cache.put(
                CacheKey::Account(*address),
                CacheValue::Account(account.clone())
            );
            
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }
    
    /// 读取 topic 的最新条目
    /// 
    /// TODO: 性能优化 - 使用单独的索引 topic 记录最新位置
    fn read_latest_for_topic(&self, topic: &str) -> Result<Option<Entry>, DbError> {
        // 批量读取该 topic 的所有条目（简化实现）
        // 生产环境应该维护索引避免全表扫描
        let max_bytes = 1024 * 1024 * 10; // 10MB
        let entries = self.wal.batch_read_for_topic(topic, max_bytes, false, None)
            .map_err(|e| DbError::Walrus(e.to_string()))?;
        
        // 返回最后一条
        Ok(entries.into_iter().last())
    }
    
    /// 持久化存储槽
    fn persist_storage(&self, address: &Address, key: U256, value: U256) -> Result<(), DbError> {
        let topic = format!("storage:{}:{}", hex::encode(address), key);
        let data = bincode::serialize(&value)
            .map_err(|e| DbError::Serialization(e.to_string()))?;
        
        self.wal.append_for_topic(&topic, &data)
            .map_err(|e| DbError::Walrus(e.to_string()))?;
        
        Ok(())
    }
    
    /// 加载存储槽
    fn load_storage(&self, address: &Address, key: U256) -> Result<U256, DbError> {
        // 1. 尝试从缓存读取
        if let Some(cached) = self.cache.get(&CacheKey::Storage(*address, key)) {
            return Ok(cached.as_storage());
        }
        
        // 2. 从 Walrus 读取
        let topic = format!("storage:{}:{}", hex::encode(address), key);
        
        if let Some(entry) = self.read_latest_for_topic(&topic)? {
            let value: U256 = bincode::deserialize(&entry.data)
                .map_err(|e| DbError::Serialization(e.to_string()))?;
            
            // 3. 更新缓存
            self.cache.put(
                CacheKey::Storage(*address, key),
                CacheValue::Storage(value)
            );
            
            Ok(value)
        } else {
            Ok(U256::ZERO)
        }
    }
    
    /// 追踪变更的账户
    fn track_changed_account(&self, address: Address) {
        let mut changed = self.changed_accounts.write();
        if !changed.contains(&address) {
            changed.push(address);
        }
    }
}

impl StateDatabase for WalrusStateDB {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, DbError> {
        // 1. 检查事务缓冲区
        if let Some(ref buffer) = *self.tx_buffer.read() {
            if let Some(account) = buffer.accounts.get(address) {
                return Ok(Some(account.clone()));
            }
            if buffer.deleted_accounts.contains(address) {
                return Ok(None);
            }
        }
        
        // 2. 从存储加载
        self.load_account(address)
    }
    
    fn set_account(&mut self, address: &Address, account: Account) -> Result<(), DbError> {
        // 追踪变更
        self.track_changed_account(*address);
        
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            // 事务模式：写入缓冲区
            buffer.accounts.insert(*address, account);
        } else {
            // 直接模式：立即持久化
            self.persist_account(address, &account)?;
            
            // 更新缓存
            self.cache.put(
                CacheKey::Account(*address),
                CacheValue::Account(account)
            );
        }
        
        Ok(())
    }
    
    fn delete_account(&mut self, address: &Address) -> Result<(), DbError> {
        self.track_changed_account(*address);
        
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.accounts.remove(address);
            buffer.deleted_accounts.push(*address);
        } else {
            // 直接删除：写入空账户
            self.persist_account(address, &Account::default())?;
            self.cache.remove(&CacheKey::Account(*address));
        }
        
        Ok(())
    }
    
    fn get_storage(&self, address: &Address, key: U256) -> Result<U256, DbError> {
        // 1. 检查事务缓冲区
        if let Some(ref buffer) = *self.tx_buffer.read() {
            if let Some(value) = buffer.storage.get(&(*address, key)) {
                return Ok(*value);
            }
        }
        
        // 2. 从存储加载
        self.load_storage(address, key)
    }
    
    fn set_storage(&mut self, address: &Address, key: U256, value: U256) -> Result<(), DbError> {
        self.track_changed_account(*address);
        
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.storage.insert((*address, key), value);
        } else {
            self.persist_storage(address, key, value)?;
            self.cache.put(
                CacheKey::Storage(*address, key),
                CacheValue::Storage(value)
            );
        }
        
        Ok(())
    }
    
    fn get_all_storage(&self, _address: &Address) -> Result<Vec<StorageSlot>, DbError> {
        // TODO: 实现存储槽扫描（需要索引支持）
        // 当前简化实现：返回空
        Ok(Vec::new())
    }
    
    fn get_code(&self, code_hash: &B256) -> Result<Option<Bytes>, DbError> {
        // 1. 检查缓存
        if let Some(cached) = self.cache.get(&CacheKey::Code(*code_hash)) {
            return Ok(Some(cached.as_code().clone()));
        }
        
        // 2. 从 Walrus 读取
        let topic = format!("code:{}", hex::encode(code_hash));
        
        if let Some(entry) = self.read_latest_for_topic(&topic)? {
            let code = Bytes::from(entry.data);
            self.cache.put(
                CacheKey::Code(*code_hash),
                CacheValue::Code(code.clone())
            );
            Ok(Some(code))
        } else {
            Ok(None)
        }
    }
    
    fn set_code(&mut self, code_hash: B256, code: Bytes) -> Result<(), DbError> {
        let topic = format!("code:{}", hex::encode(code_hash));
        
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.codes.insert(code_hash, code);
        } else {
            self.wal.append_for_topic(&topic, &code)
                .map_err(|e| DbError::Walrus(e.to_string()))?;
            
            self.cache.put(
                CacheKey::Code(code_hash),
                CacheValue::Code(code)
            );
        }
        
        Ok(())
    }
    
    fn get_block_hash(&self, block_number: u64) -> Result<Option<B256>, DbError> {
        if let Some(cached) = self.cache.get(&CacheKey::BlockHash(block_number)) {
            return Ok(Some(cached.as_block_hash()));
        }
        
        let topic = format!("block_hash:{}", block_number);
        
        if let Some(entry) = self.read_latest_for_topic(&topic)? {
            if entry.data.len() == 32 {
                let hash = B256::from_slice(&entry.data);
                self.cache.put(
                    CacheKey::BlockHash(block_number),
                    CacheValue::BlockHash(hash)
                );
                Ok(Some(hash))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    fn set_block_hash(&mut self, block_number: u64, block_hash: B256) -> Result<(), DbError> {
        let topic = format!("block_hash:{}", block_number);
        
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.block_hashes.insert(block_number, block_hash);
        } else {
            self.wal.append_for_topic(&topic, block_hash.as_slice())
                .map_err(|e| DbError::Walrus(e.to_string()))?;
            
            self.cache.put(
                CacheKey::BlockHash(block_number),
                CacheValue::BlockHash(block_hash)
            );
        }
        
        Ok(())
    }
    
    fn begin_transaction(&mut self) -> Result<(), DbError> {
        let mut buffer = self.tx_buffer.write();
        if buffer.is_some() {
            return Err(DbError::Transaction("Transaction already started".to_string()));
        }
        *buffer = Some(TransactionBuffer::new());
        
        // 清空变更追踪
        self.changed_accounts.write().clear();
        
        Ok(())
    }
    
    fn commit_transaction(&mut self) -> Result<(), DbError> {
        let mut buffer_guard = self.tx_buffer.write();
        let buffer = buffer_guard.take()
            .ok_or_else(|| DbError::Transaction("No active transaction".to_string()))?;
        
        // 持久化所有变更
        for (address, account) in buffer.accounts {
            self.persist_account(&address, &account)?;
            self.cache.put(
                CacheKey::Account(address),
                CacheValue::Account(account)
            );
        }
        
        for ((address, key), value) in buffer.storage {
            self.persist_storage(&address, key, value)?;
            self.cache.put(
                CacheKey::Storage(address, key),
                CacheValue::Storage(value)
            );
        }
        
        for (code_hash, code) in buffer.codes {
            let topic = format!("code:{}", hex::encode(code_hash));
            self.wal.append_for_topic(&topic, &code)
                .map_err(|e| DbError::Walrus(e.to_string()))?;
            self.cache.put(
                CacheKey::Code(code_hash),
                CacheValue::Code(code)
            );
        }
        
        for (block_number, block_hash) in buffer.block_hashes {
            let topic = format!("block_hash:{}", block_number);
            self.wal.append_for_topic(&topic, block_hash.as_slice())
                .map_err(|e| DbError::Walrus(e.to_string()))?;
            self.cache.put(
                CacheKey::BlockHash(block_number),
                CacheValue::BlockHash(block_hash)
            );
        }
        
        Ok(())
    }
    
    fn rollback_transaction(&mut self) -> Result<(), DbError> {
        let mut buffer = self.tx_buffer.write();
        if buffer.is_none() {
            return Err(DbError::Transaction("No active transaction".to_string()));
        }
        *buffer = None;
        
        // 清空变更追踪
        self.changed_accounts.write().clear();
        
        Ok(())
    }
    
    fn get_changed_accounts(&self) -> Result<Vec<Address>, DbError> {
        Ok(self.changed_accounts.read().clone())
    }
    
    fn clear_cache(&mut self) -> Result<(), DbError> {
        self.cache.clear();
        Ok(())
    }
}

impl Default for WalrusStateDB {
    fn default() -> Self {
        Self::new().expect("Failed to create WalrusStateDB")
    }
}
