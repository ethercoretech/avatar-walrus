//! Redb 状态数据库实现
//! 
//! 使用嵌入式 redb 数据库进行本地持久化存储

use redb::{Database, TableDefinition, ReadableTable, ReadableDatabase};
use alloy_primitives::{Address, B256, U256, Bytes};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::db::{StateDatabase, DbError, TransactionBuffer};
use crate::schema::{Account, StorageSlot, Block, EMPTY_CODE_HASH};
use crate::wallet::get_builtin_wallets;

// ==================== 表定义 ====================

/// 账户表: address (20 bytes) -> account data
const ACCOUNTS_TABLE: TableDefinition<&[u8; 20], &[u8]> = 
    TableDefinition::new("accounts");

/// 存储表: (address (20 bytes), key (32 bytes)) -> value (32 bytes)
const STORAGE_TABLE: TableDefinition<(&[u8; 20], &[u8; 32]), &[u8; 32]> = 
    TableDefinition::new("storage");

/// 代码表: code_hash (32 bytes) -> bytecode
const CODE_TABLE: TableDefinition<&[u8; 32], &[u8]> = 
    TableDefinition::new("code");

/// 区块表: block_number -> block data
const BLOCKS_TABLE: TableDefinition<u64, &[u8]> = 
    TableDefinition::new("blocks");

/// 区块哈希表: block_number -> block_hash (32 bytes)
const BLOCK_HASHES_TABLE: TableDefinition<u64, &[u8; 32]> = 
    TableDefinition::new("block_hashes");

// ==================== RedbStateDB ====================

/// 基于 Redb 的状态数据库
/// 
/// 提供本地持久化存储，支持账户、存储、代码和区块的读写
pub struct RedbStateDB {
    /// Redb 数据库实例
    db: Arc<Database>,
    
    /// 事务缓冲区（内存中暂存未提交的变更）
    tx_buffer: RwLock<Option<TransactionBuffer>>,
    
    /// 变更追踪（用于状态根计算）
    changed_accounts: RwLock<Vec<Address>>,
}

impl RedbStateDB {
    /// 创建或打开 Redb 数据库（读写模式）
    pub fn new(path: &str) -> Result<Self, DbError> {
        Self::open_internal(path, false)
    }

    /// 以只读模式打开 Redb 数据库（用于 rpc-gateway 等只读访问）
    pub fn open_readonly(path: &str) -> Result<Self, DbError> {
        Self::open_internal(path, true)
    }

    /// 内部打开方法
    fn open_internal(path: &str, readonly: bool) -> Result<Self, DbError> {
        // 确保父目录存在
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Io(e))?;
        }
        
        // 创建或打开数据库
        let db = if readonly {
            // 只读模式：使用 Database::open 打开已有数据库
            Database::open(path)
                .map_err(|e| DbError::Other(format!("Failed to open database (readonly): {}", e)))?
        } else {
            // 读写模式：创建或打开数据库
            Database::create(path)
                .map_err(|e| DbError::Other(format!("Failed to create database: {}", e)))?
        };
        
        // 初始化所有表（只在读写模式下）
        if !readonly {
            let write_txn = db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let _ = write_txn.open_table(ACCOUNTS_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let _ = write_txn.open_table(STORAGE_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let _ = write_txn.open_table(CODE_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let _ = write_txn.open_table(BLOCKS_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let _ = write_txn.open_table(BLOCK_HASHES_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))?;
        }

        let db_instance = Self {
            db: Arc::new(db),
            tx_buffer: RwLock::new(None),
            changed_accounts: RwLock::new(Vec::new()),
        };
        
        // 初始化内置钱包账户（只在读写模式下）
        if !readonly {
            db_instance.initialize_builtin_accounts()?;
        }

        Ok(db_instance)
    }

    /// 持久化区块到数据库
    /// 
    /// 在区块执行完成后调用此方法，将区块头和交易存储到磁盘
    pub fn save_block(&self, block: &Block) -> Result<(), DbError> {
        let write_txn = self.db.begin_write()
            .map_err(|e| DbError::Other(e.to_string()))?;
        {
            let mut table = write_txn.open_table(BLOCKS_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            let data = bincode::serialize(block)
                .map_err(|e| DbError::Serialization(e.to_string()))?;
            table.insert(block.header.number, data.as_slice())
                .map_err(|e| DbError::Other(e.to_string()))?;
        }
        write_txn.commit()
            .map_err(|e| DbError::Other(e.to_string()))?;
        Ok(())
    }

    /// 从数据库读取区块
    pub fn get_block(&self, block_number: u64) -> Result<Option<Block>, DbError> {
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(BLOCKS_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        let value = table.get(block_number)
            .map_err(|e| DbError::Other(e.to_string()))?;
        
        if let Some(data) = value {
            let block: Block = bincode::deserialize(data.value())
                .map_err(|e| DbError::Serialization(e.to_string()))?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// 追踪变更的账户
    fn track_changed_account(&self, address: Address) {
        let mut changed = self.changed_accounts.write();
        if !changed.contains(&address) {
            changed.push(address);
        }
    }
    /// 初始化内置钱包账户
    /// 
    /// 为所有预配置的测试账户创建初始余额
    fn initialize_builtin_accounts(&self) -> Result<(), DbError> {
        let wallets = get_builtin_wallets();
        
        let write_txn = self.db.begin_write()
            .map_err(|e| DbError::Other(e.to_string()))?;
        {
            let mut table = write_txn.open_table(ACCOUNTS_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            
            for wallet in wallets {
                let address_bytes: [u8; 20] = wallet.address.into();
                
                // 检查账户是否已存在
                let exists = table.get(&address_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?
                    .is_some();
                
                if !exists {
                    // 创建新账户
                    let account = Account {
                        balance: wallet.initial_balance_wei(),
                        nonce: 0,
                        code_hash: EMPTY_CODE_HASH,
                        storage_root: B256::ZERO,
                    };
                    
                    let data = bincode::serialize(&account)
                        .map_err(|e| DbError::Serialization(e.to_string()))?;
                    
                    table.insert(&address_bytes, data.as_slice())
                        .map_err(|e| DbError::Other(e.to_string()))?;
                    
                    tracing::info!("💰 初始化内置账户: {:?}, 余额: {} ETH", 
                        wallet.address, 
                        wallet.initial_balance_eth);
                }
            }
        }
        write_txn.commit()
            .map_err(|e| DbError::Other(e.to_string()))?;
        
        Ok(())
    }
}

// ==================== StateDatabase 实现 ====================

impl StateDatabase for RedbStateDB {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, DbError> {
        // 1. 先检查事务缓冲区
        if let Some(ref buffer) = *self.tx_buffer.read() {
            if let Some(acc) = buffer.accounts.get(address) {
                return Ok(Some(acc.clone()));
            }
            if buffer.deleted_accounts.contains(address) {
                return Ok(None);
            }
        }

        // 2. 从 Redb 读取
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(ACCOUNTS_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
        let value = table.get(&addr_bytes)
            .map_err(|e| DbError::Other(e.to_string()))?;
        
        if let Some(data) = value {
            let account: Account = bincode::deserialize(data.value())
                .map_err(|e| DbError::Serialization(e.to_string()))?;
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }

    fn set_account(&mut self, address: &Address, account: Account) -> Result<(), DbError> {
        self.track_changed_account(*address);

        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            // 事务模式：写入缓冲区
            buffer.accounts.insert(*address, account);
            Ok(())
        } else {
            // 直接模式：立即持久化
            let write_txn = self.db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let mut table = write_txn.open_table(ACCOUNTS_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let data = bincode::serialize(&account)
                    .map_err(|e| DbError::Serialization(e.to_string()))?;
                let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
                table.insert(&addr_bytes, data.as_slice())
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))
        }
    }

    fn delete_account(&mut self, address: &Address) -> Result<(), DbError> {
        self.track_changed_account(*address);

        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.accounts.remove(address);
            buffer.deleted_accounts.push(*address);
            Ok(())
        } else {
            let write_txn = self.db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let mut table = write_txn.open_table(ACCOUNTS_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
                table.remove(&addr_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))
        }
    }

    fn get_storage(&self, address: &Address, key: U256) -> Result<U256, DbError> {
        // 1. 检查事务缓冲区
        if let Some(ref buffer) = *self.tx_buffer.read() {
            if let Some(val) = buffer.storage.get(&(*address, key)) {
                return Ok(*val);
            }
        }

        // 2. 从 Redb 读取
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(STORAGE_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
        let key_bytes: [u8; 32] = key.to_be_bytes();
        
        let value = table.get((&addr_bytes, &key_bytes))
            .map_err(|e| DbError::Other(e.to_string()))?;
        if let Some(data) = value {
            Ok(U256::from_be_bytes(*data.value()))
        } else {
            Ok(U256::ZERO)
        }
    }

    fn set_storage(&mut self, address: &Address, key: U256, value: U256) -> Result<(), DbError> {
        self.track_changed_account(*address);

        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.storage.insert((*address, key), value);
            Ok(())
        } else {
            let write_txn = self.db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let mut table = write_txn.open_table(STORAGE_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
                let key_bytes: [u8; 32] = key.to_be_bytes();
                let val_bytes: [u8; 32] = value.to_be_bytes();
                table.insert((&addr_bytes, &key_bytes), &val_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))
        }
    }

    fn get_all_storage(&self, address: &Address) -> Result<Vec<StorageSlot>, DbError> {
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(STORAGE_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        
        let addr_bytes: [u8; 20] = address.as_slice().try_into().unwrap();
        let mut slots = Vec::new();
        
        // 迭代所有条目，过滤出属于该地址的存储槽
        let iter = table.iter()
            .map_err(|e| DbError::Other(e.to_string()))?;
        
        for item in iter {
            let (key, value) = item.map_err(|e| DbError::Other(e.to_string()))?;
            let (key_addr, key_slot) = key.value();
            
            // 检查地址是否匹配
            if key_addr == &addr_bytes {
                let slot_key = U256::from_be_bytes(*key_slot);
                let slot_value = U256::from_be_bytes(*value.value());
                slots.push(StorageSlot {
                    address: *address,
                    key: slot_key,
                    value: slot_value,
                });
            }
        }
        
        Ok(slots)
    }

    fn get_code(&self, code_hash: &B256) -> Result<Option<Bytes>, DbError> {
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(CODE_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        let hash_bytes: [u8; 32] = code_hash.as_slice().try_into().unwrap();
        let value = table.get(&hash_bytes)
            .map_err(|e| DbError::Other(e.to_string()))?;
        Ok(value.map(|d| Bytes::copy_from_slice(d.value())))
    }

    fn set_code(&mut self, code_hash: B256, code: Bytes) -> Result<(), DbError> {
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.codes.insert(code_hash, code);
            Ok(())
        } else {
            let write_txn = self.db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let mut table = write_txn.open_table(CODE_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let hash_bytes: [u8; 32] = code_hash.as_slice().try_into().unwrap();
                table.insert(&hash_bytes, code.as_ref())
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))
        }
    }

    fn get_block_hash(&self, block_number: u64) -> Result<Option<B256>, DbError> {
        // 1. 检查事务缓冲区
        if let Some(ref buffer) = *self.tx_buffer.read() {
            if let Some(hash) = buffer.block_hashes.get(&block_number) {
                return Ok(Some(*hash));
            }
        }

        // 2. 从 Redb 读取
        let read_txn = self.db.begin_read()
            .map_err(|e| DbError::Other(e.to_string()))?;
        let table = read_txn.open_table(BLOCK_HASHES_TABLE)
            .map_err(|e| DbError::Other(e.to_string()))?;
        let value = table.get(block_number)
            .map_err(|e| DbError::Other(e.to_string()))?;
        Ok(value.map(|d: redb::AccessGuard<&[u8; 32]>| B256::from_slice(&*d.value())))
    }

    fn set_block_hash(&mut self, block_number: u64, block_hash: B256) -> Result<(), DbError> {
        if let Some(ref mut buffer) = *self.tx_buffer.write() {
            buffer.block_hashes.insert(block_number, block_hash);
            Ok(())
        } else {
            let write_txn = self.db.begin_write()
                .map_err(|e| DbError::Other(e.to_string()))?;
            {
                let mut table = write_txn.open_table(BLOCK_HASHES_TABLE)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let hash_bytes: [u8; 32] = block_hash.as_slice().try_into().unwrap();
                table.insert(block_number, &hash_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            write_txn.commit()
                .map_err(|e| DbError::Other(e.to_string()))
        }
    }

    fn begin_transaction(&mut self) -> Result<(), DbError> {
        let mut buffer = self.tx_buffer.write();
        if buffer.is_some() {
            return Err(DbError::Transaction("Transaction already started".to_string()));
        }
        *buffer = Some(TransactionBuffer::new());
        self.changed_accounts.write().clear();
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), DbError> {
        let mut buffer_guard = self.tx_buffer.write();
        let buffer = buffer_guard.take()
            .ok_or_else(|| DbError::Transaction("No active transaction".to_string()))?;

        let write_txn = self.db.begin_write()
            .map_err(|e| DbError::Other(e.to_string()))?;
        {
            // 写入账户变更
            let mut accounts = write_txn.open_table(ACCOUNTS_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            for (addr, acc) in buffer.accounts {
                let data = bincode::serialize(&acc)
                    .map_err(|e| DbError::Serialization(e.to_string()))?;
                let addr_bytes: [u8; 20] = addr.as_slice().try_into().unwrap();
                accounts.insert(&addr_bytes, data.as_slice())
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }

            // 删除账户
            for addr in buffer.deleted_accounts {
                let addr_bytes: [u8; 20] = addr.as_slice().try_into().unwrap();
                accounts.remove(&addr_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
            
            // 写入存储变更
            let mut storage = write_txn.open_table(STORAGE_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            for ((addr, key), val) in buffer.storage {
                let addr_bytes: [u8; 20] = addr.as_slice().try_into().unwrap();
                let key_bytes: [u8; 32] = key.to_be_bytes();
                let val_bytes: [u8; 32] = val.to_be_bytes();
                storage.insert((&addr_bytes, &key_bytes), &val_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }

            // 写入代码
            let mut codes = write_txn.open_table(CODE_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            for (code_hash, code) in buffer.codes {
                let hash_bytes: [u8; 32] = code_hash.as_slice().try_into().unwrap();
                codes.insert(&hash_bytes, code.as_ref())
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }

            // 写入区块哈希
            let mut block_hashes = write_txn.open_table(BLOCK_HASHES_TABLE)
                .map_err(|e| DbError::Other(e.to_string()))?;
            for (block_number, block_hash) in buffer.block_hashes {
                let hash_bytes: [u8; 32] = block_hash.as_slice().try_into().unwrap();
                block_hashes.insert(block_number, &hash_bytes)
                    .map_err(|e| DbError::Other(e.to_string()))?;
            }
        }
        write_txn.commit()
            .map_err(|e| DbError::Other(e.to_string()))
    }

    fn rollback_transaction(&mut self) -> Result<(), DbError> {
        *self.tx_buffer.write() = None;
        self.changed_accounts.write().clear();
        Ok(())
    }

    fn get_changed_accounts(&self) -> Result<Vec<Address>, DbError> {
        Ok(self.changed_accounts.read().clone())
    }

    fn clear_cache(&mut self) -> Result<(), DbError> {
        // RedbStateDB 不使用额外的缓存（事务缓冲区除外）
        Ok(())
    }
}

impl Default for RedbStateDB {
    fn default() -> Self {
        Self::new("./data/state.redb").expect("Failed to create RedbStateDB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use alloy_primitives::address;
    
    fn create_test_db() -> (RedbStateDB, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        (db, temp_dir)
    }
    
    #[test]
    fn test_account_crud() {
        let (mut db, _temp_dir) = create_test_db();
        
        let addr = address!("0000000000000000000000000000000000000001");
        let account = Account::with_balance(U256::from(1000));
        
        // 测试写入
        db.set_account(&addr, account.clone()).unwrap();
        
        // 测试读取
        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved, Some(account));
        
        // 测试删除
        db.delete_account(&addr).unwrap();
        let deleted = db.get_account(&addr).unwrap();
        assert_eq!(deleted, None);
    }
    
    #[test]
    fn test_storage_crud() {
        let (mut db, _temp_dir) = create_test_db();
        
        let addr = address!("0000000000000000000000000000000000000002");
        let key = U256::from(42);
        let value = U256::from(12345);
        
        // 测试写入
        db.set_storage(&addr, key, value).unwrap();
        
        // 测试读取
        let retrieved = db.get_storage(&addr, key).unwrap();
        assert_eq!(retrieved, value);
        
        // 测试默认值
        let non_existent = db.get_storage(&addr, U256::from(999)).unwrap();
        assert_eq!(non_existent, U256::ZERO);
        
        // get_all_storage 应该返回一个非空列表并包含刚写入的槽位
        let mut all = db.get_all_storage(&addr).unwrap();
        all.sort_by_key(|slot| slot.key);
        assert!(!all.is_empty());
        assert_eq!(all[0].key, key);
        assert_eq!(all[0].value, value);
    }
    
    #[test]
    fn test_transaction() {
        let (mut db, _temp_dir) = create_test_db();
        
        let addr = address!("0000000000000000000000000000000000000003");
        let account = Account::with_balance(U256::from(1000));
        
        // 开始事务
        db.begin_transaction().unwrap();
        
        // 在事务中修改
        db.set_account(&addr, account.clone()).unwrap();
        
        // 事务中可以读取
        let in_tx = db.get_account(&addr).unwrap();
        assert_eq!(in_tx, Some(account.clone()));
        
        // 提交事务
        db.commit_transaction().unwrap();
        
        // 事务外可以读取
        let after_commit = db.get_account(&addr).unwrap();
        assert_eq!(after_commit, Some(account));
    }
    
    #[test]
    fn test_transaction_rollback() {
        let (mut db, _temp_dir) = create_test_db();
        
        let addr = address!("0000000000000000000000000000000000000004");
        let account = Account::with_balance(U256::from(1000));
        
        // 开始事务
        db.begin_transaction().unwrap();
        
        // 在事务中修改
        db.set_account(&addr, account.clone()).unwrap();
        
        // 回滚事务
        db.rollback_transaction().unwrap();
        
        // 回滚后不应该有数据
        let after_rollback = db.get_account(&addr).unwrap();
        assert_eq!(after_rollback, None);
    }
    
    #[test]
    fn test_changed_accounts_tracking() {
        let (mut db, _temp_dir) = create_test_db();
        
        let addr1 = address!("0000000000000000000000000000000000000005");
        let addr2 = address!("0000000000000000000000000000000000000006");
        
        db.begin_transaction().unwrap();
        
        db.set_account(&addr1, Account::with_balance(U256::from(100))).unwrap();
        db.set_account(&addr2, Account::with_balance(U256::from(200))).unwrap();
        
        let changed = db.get_changed_accounts().unwrap();
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&addr1));
        assert!(changed.contains(&addr2));
        
        db.commit_transaction().unwrap();
    }

    #[test]
    fn test_code_crud() {
        use alloy_primitives::keccak256;

        let (mut db, _temp_dir) = create_test_db();
        let code = Bytes::from(vec![0x60, 0x01, 0x60, 0x00, 0x52]); // 简单的 EVM bytecode 片段
        let hash = keccak256(code.as_ref());

        // 初始应为 None
        assert!(db.get_code(&hash).unwrap().is_none());

        // 设置代码
        db.set_code(hash, code.clone()).unwrap();

        // 可以读回相同字节码
        let retrieved = db.get_code(&hash).unwrap().expect("code should exist");
        assert_eq!(retrieved, code);
    }

    #[test]
    fn test_block_and_block_hash_crud() {
        use crate::schema::{BlockHeader, Block};
        use chrono::Utc;

        let (db, _temp_dir) = create_test_db();
        let mut db = db;

        // 构造一个简单区块
        let header = BlockHeader {
            number: 1,
            parent_hash: "0x0".to_string(),
            timestamp: Utc::now(),
            tx_count: 0,
            transactions_root: "0x00".to_string(),
            state_root: None,
            gas_used: None,
            gas_limit: Some(30_000_000),
            receipts_root: None,
        };
        let block = Block {
            header,
            transactions: Vec::new(),
        };

        // 将区块哈希的 0x 开头十六进制字符串解析为 B256
        let hash_str = block.hash();
        let hash_hex = hash_str.trim_start_matches("0x");
        let hash_bytes = hex::decode(hash_hex).expect("valid hex block hash");
        let hash = B256::from_slice(&hash_bytes);

        // 初始应无区块和区块哈希
        assert!(db.get_block(1).unwrap().is_none());
        assert!(db.get_block_hash(1).unwrap().is_none());

        // 保存区块与区块哈希
        db.save_block(&block).unwrap();
        db.set_block_hash(1, hash).unwrap();

        // 可以读回区块与哈希
        let got_block = db.get_block(1).unwrap().expect("block should exist");
        assert_eq!(got_block.header.number, 1);
        let got_hash = db.get_block_hash(1).unwrap().expect("block hash should exist");
        assert_eq!(got_hash, hash);
    }
}
