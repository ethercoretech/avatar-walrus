//! REVM Database Trait 适配器
//! 
//! 将 RedbStateDB 适配为 REVM 的 Database trait，并提供 EVM 执行环境

use alloy_primitives::{Address, U256, B256};
use revm::{
    primitives::{
        AccountInfo, Bytecode, BlockEnv, TxEnv, 
        ExecutionResult as RevmExecutionResult, Output, SpecId,
    },
    Database, Evm,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;

use crate::db::{StateDatabase, RedbStateDB, DbError};
use crate::schema::account::EMPTY_CODE_HASH;
use crate::executor::{ExecutorError, ExecutionResult};

/// 带缓存的 Redb 状态包装器
/// 
/// 为 REVM 提供 Database trait 实现，包含内存缓存优化读取性能
pub struct CachedRedbState {
    /// 数据库引用（使用 Arc + RwLock 支持并发访问）
    db: Arc<RwLock<RedbStateDB>>,
    
    /// 账户信息缓存（减少数据库访问）
    cache: RwLock<HashMap<Address, AccountInfo>>,
}

impl CachedRedbState {
    /// 创建新的缓存状态
    pub fn new(db: Arc<RwLock<RedbStateDB>>) -> Self {
        Self {
            db,
            cache: RwLock::new(HashMap::new()),
        }
    }
    
    /// 清除缓存
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }
}

impl Database for CachedRedbState {
    type Error = DbError;
    
    /// 获取账户基本信息
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // 1. 检查缓存
        if let Some(info) = self.cache.read().get(&address) {
            return Ok(Some(info.clone()));
        }
        
        // 2. 从数据库读取
        let db = self.db.read();
        let account = db.get_account(&address)?;
        
        // 3. 构建账户信息（如果不存在则返回默认空账户）
        let info = if let Some(acc) = account {
            AccountInfo {
                balance: acc.balance,
                nonce: acc.nonce,
                code_hash: acc.code_hash,
                code: None, // 延迟加载代码
            }
        } else {
            // 返回默认空账户（余额为0，nonce为0，空代码哈希）
            AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: EMPTY_CODE_HASH,
                code: None,
            }
        };
        
        // 4. 更新缓存
        self.cache.write().insert(address, info.clone());
        
        Ok(Some(info))
    }
    
    /// 根据哈希获取合约字节码
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // 空代码哈希直接返回空字节码
        if code_hash == EMPTY_CODE_HASH {
            return Ok(Bytecode::new());
        }
        
        let db = self.db.read();
        let code = db.get_code(&code_hash)?
            .ok_or_else(|| DbError::CodeNotFound(code_hash))?;
        
        // 将字节码转换为 REVM 的 Bytecode 类型
        Ok(Bytecode::new_raw(code))
    }
    
    /// 获取存储槽值
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let db = self.db.read();
        db.get_storage(&address, index)
    }
    
    /// 获取区块哈希
    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        let db = self.db.read();
        db.get_block_hash(number)?
            .ok_or_else(|| DbError::BlockNotFound(number))
    }
}

/// REVM 适配器
/// 
/// 封装 EVM 执行引擎，提供交易执行接口
pub struct RevmAdapter {
    /// 数据库引用
    db: Arc<RwLock<RedbStateDB>>,
    
    /// EVM 实例（使用 CachedRedbState 作为数据库后端）
    evm: Evm<'static, (), CachedRedbState>,
}

impl RevmAdapter {
    /// 创建新的适配器
    pub fn new(db: RedbStateDB) -> Self {
        let db_arc = Arc::new(RwLock::new(db));
        let cached_state = CachedRedbState::new(Arc::clone(&db_arc));
        
        // 构建 EVM 实例 - 使用 Shanghai 规范避免 EIP-3607
        let evm = Evm::builder()
            .with_db(cached_state)
            .with_spec_id(SpecId::SHANGHAI) // 使用 Shanghai 规范（在 EIP-3607 之前）
            .build();
        
        Self {
            db: db_arc,
            evm,
        }
    }
    
    /// 执行交易
    /// 
    /// 将交易数据转换为 REVM TxEnv，执行后返回结果
    pub fn execute(
        &mut self,
        tx_env: TxEnv,
        block_env: BlockEnv,
    ) -> Result<ExecutionResult, ExecutorError> {
        // 设置环境
        self.evm.context.evm.env.block = block_env;
        self.evm.context.evm.env.tx = tx_env;
        
        // 执行交易
        let result_and_state = self.evm.transact()
            .map_err(|e| ExecutorError::Evm(format!("{:?}", e)))?;
        
        // 应用状态变更到数据库
        self.apply_state_changes(&result_and_state)?;
        
        // 清除缓存，确保下次交易读取到最新状态（特别是 nonce）
        self.evm.context.evm.db.clear_cache();
        
        // 转换执行结果
        self.convert_result(result_and_state.result)
    }
    
    /// 应用状态变更到数据库
    /// 
    /// 将 REVM 的状态变更（BundleState）写入 RedbStateDB
    fn apply_state_changes(
        &mut self,
        result: &revm::primitives::result::ResultAndState,
    ) -> Result<(), ExecutorError> {
        let mut db = self.db.write();
        
        // 遍历状态变更
        for (address, account) in &result.state {
            if account.is_selfdestructed() {
                // 账户被销毁
                db.delete_account(address)
                    .map_err(|e| ExecutorError::Database(e.to_string()))?;
            } else if account.is_touched() {
                // 账户信息变更
                let info = &account.info;
                let mut acc = crate::schema::Account::default();
                acc.balance = info.balance;
                acc.nonce = info.nonce;
                acc.code_hash = info.code_hash;
                
                db.set_account(address, acc)
                    .map_err(|e| ExecutorError::Database(e.to_string()))?;
                
                // 存储合约字节码（REVM 12 关键逻辑）
                // 当 account.info.code 有值且 code_hash 有效时，需要持久化字节码
                if let Some(ref code) = info.code {
                    // 避免存储空代码或空哈希
                    if info.code_hash != EMPTY_CODE_HASH && !code.is_empty() {
                        db.set_code(info.code_hash, code.bytes().clone())
                            .map_err(|e| ExecutorError::Database(e.to_string()))?;
                    }
                }
                
                // 存储槽变更
                for (slot, value) in &account.storage {
                    if value.is_changed() {
                        db.set_storage(address, *slot, value.present_value())
                            .map_err(|e| ExecutorError::Database(e.to_string()))?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 转换 REVM 执行结果为通用格式
    fn convert_result(
        &self,
        result: RevmExecutionResult,
    ) -> Result<ExecutionResult, ExecutorError> {
        match result {
            RevmExecutionResult::Success { output, gas_used, gas_refunded, logs, .. } => {
                let (out, contract_address) = match output {
                    Output::Call(data) => (Some(data), None),
                    Output::Create(data, addr) => (Some(data), addr),
                };
                
                Ok(ExecutionResult {
                    gas_used,
                    success: true,
                    output: out,
                    contract_address,
                    gas_refund: gas_refunded,
                    logs,
                })
            }
            RevmExecutionResult::Revert { output, gas_used } => {
                Ok(ExecutionResult {
                    gas_used,
                    success: false,
                    output: Some(output),
                    contract_address: None,
                    gas_refund: 0,
                    logs: Vec::new(),
                })
            }
            RevmExecutionResult::Halt { reason, gas_used } => {
                Err(ExecutorError::Evm(format!("EVM Halted: {:?}, gas_used: {}", reason, gas_used)))
            }
        }
    }
    
    /// 获取内部数据库的可变引用
    pub fn db_mut(&mut self) -> &mut RedbStateDB {
        // 注意：这里需要临时获取锁，返回可变引用会有生命周期问题
        // 实际使用中应该通过 Arc<RwLock> 直接操作
        unsafe {
            let ptr = self.db.as_ref() as *const RwLock<RedbStateDB> as *mut RwLock<RedbStateDB>;
            &mut *(*ptr).get_mut()
        }
    }
    
    /// 获取内部数据库的不可变引用
    pub fn db(&self) -> &RedbStateDB {
        // 使用 unsafe 以绕过 RwLock
        unsafe {
            let ptr = self.db.as_ref() as *const RwLock<RedbStateDB>;
            &*(*ptr).data_ptr()
        }
    }
    
    /// 获取数据库的 Arc 引用
    pub fn db_arc(&self) -> Arc<RwLock<RedbStateDB>> {
        Arc::clone(&self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Account;
    use crate::db::StateDatabase;
    use alloy_primitives::{address, Bytes};
    use revm::primitives::TransactTo;
    use tempfile::TempDir;
    
    fn create_test_db() -> (RedbStateDB, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        (db, temp_dir)
    }
    
    #[test]
    fn test_cached_redb_state_basic() {
        let (db, _temp_dir) = create_test_db();
        let db_arc = Arc::new(RwLock::new(db));
        let mut cached_state = CachedRedbState::new(db_arc.clone());
        
        let addr = address!("0000000000000000000000000000000000000001");
        let account = Account::with_balance(U256::from(1000));
        
        // 设置账户
        db_arc.write().set_account(&addr, account).unwrap();
        
        // 通过适配器读取
        let info = cached_state.basic(addr).unwrap().unwrap();
        
        assert_eq!(info.balance, U256::from(1000));
        assert_eq!(info.nonce, 0);
        
        // 第二次读取应该命中缓存
        let info2 = cached_state.basic(addr).unwrap().unwrap();
        assert_eq!(info2.balance, U256::from(1000));
    }
    
    #[test]
    fn test_cached_redb_state_storage() {
        let (db, _temp_dir) = create_test_db();
        let db_arc = Arc::new(RwLock::new(db));
        let mut cached_state = CachedRedbState::new(db_arc.clone());
        
        let addr = address!("0000000000000000000000000000000000000001");
        
        // 设置存储
        db_arc.write().set_storage(&addr, U256::from(1), U256::from(100)).unwrap();
        
        // 通过适配器读取
        let value = cached_state.storage(addr, U256::from(1)).unwrap();
        
        assert_eq!(value, U256::from(100));
    }
    
    #[test]
    fn test_revm_adapter_execute_simple_transfer() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置发送方账户
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let to = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
        
        let mut from_account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        from_account.nonce = 0;
        db.set_account(&from, from_account).unwrap();
        
        // 创建适配器
        let mut adapter = RevmAdapter::new(db);
        
        // 构建交易环境
        let tx_env = TxEnv {
            caller: from,
            transact_to: TransactTo::Call(to),
            value: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            data: Bytes::new().into(),
            gas_limit: 21000,
            gas_price: U256::from(1_000_000_000u64), // 1 Gwei
            nonce: Some(0),
            ..Default::default()
        };
        
        let block_env = BlockEnv::default();
        
        // 开始事务
        adapter.db_mut().begin_transaction().unwrap();
        
        // 执行交易
        let result = adapter.execute(tx_env, block_env).unwrap();
        
        // 提交事务
        adapter.db_mut().commit_transaction().unwrap();
        
        assert!(result.success);
        assert_eq!(result.gas_used, 21000);
    }
}
