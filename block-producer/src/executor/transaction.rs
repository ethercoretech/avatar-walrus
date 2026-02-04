//! 交易执行器
//! 
//! 执行单笔交易并返回结果

use alloy_primitives::{U256, Address, Bytes};
use revm::primitives::{BlockEnv, TxEnv, TransactTo};
use crate::db::{RedbStateDB, StateDatabase};
use crate::executor::{ExecutorError, RevmAdapter};
use crate::schema::Transaction;
use serde::{Deserialize, Serialize};

/// 交易执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Gas 使用量
    pub gas_used: u64,
    
    /// 执行状态（true = 成功，false = 失败）
    pub success: bool,
    
    /// 返回值或错误信息
    pub output: Option<Bytes>,
    
    /// 合约地址（合约部署交易）
    pub contract_address: Option<Address>,
    
    /// Gas 退款
    pub gas_refund: u64,
    
    /// 事件日志
    pub logs: Vec<revm::primitives::Log>,
}

/// 交易执行器
pub struct TransactionExecutor {
    adapter: RevmAdapter,
}

impl TransactionExecutor {
    /// 创建交易执行器
    pub fn new(db: RedbStateDB) -> Self {
        Self {
            adapter: RevmAdapter::new(db),
        }
    }
    
    /// 验证交易
    /// 
    /// 在执行前进行预验证,确保交易有效
    /// 
    /// # 验证项
    /// - Gas limit 非零
    /// - Nonce 有效性
    /// - 账户余额充足
    pub fn validate_transaction(&mut self, tx: &Transaction) -> Result<(), ExecutorError> {
        // 1. Gas limit 非零检查
        let gas_limit = tx.gas_limit()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        if gas_limit == 0 {
            return Err(ExecutorError::InvalidGas);
        }
        
        // 2. 获取账户信息
        let from_addr = tx.from_address()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        let account = self.adapter.db_mut()
            .get_account(&from_addr)
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        // 3. Nonce 检查
        let tx_nonce = tx.nonce_value()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        if let Some(ref acc) = account {
            if tx_nonce < acc.nonce {
                return Err(ExecutorError::NonceTooLow {
                    expected: acc.nonce,
                    got: tx_nonce,
                });
            }
        }
        
        // 4. 余额检查
        let required = Self::calculate_required_balance(tx)?;
        let available = account.map_or(U256::ZERO, |a| a.balance);
        
        if available < required {
            return Err(ExecutorError::InsufficientFunds {
                required: format!("{}", required),
                available: format!("{}", available),
            });
        }
        
        Ok(())
    }
    
    /// 计算交易所需的余额
    /// 
    /// 计算公式: gas_limit * gas_price + value
    fn calculate_required_balance(tx: &Transaction) -> Result<U256, ExecutorError> {
        let gas_limit = tx.gas_limit()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        // 解析 gas_price (如果没有提供,默认为 1 Gwei)
        let gas_price = if let Some(ref gp) = tx.gas_price {
            let hex = gp.trim_start_matches("0x");
            U256::from_str_radix(hex, 16)
                .map_err(|e| ExecutorError::Transaction(format!("Invalid gas_price: {}", e)))?
        } else {
            U256::from(1_000_000_000u64) // 1 Gwei
        };
        
        let value = tx.value_wei()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        // 计算总所需余额
        let gas_cost = U256::from(gas_limit)
            .checked_mul(gas_price)
            .ok_or_else(|| ExecutorError::Transaction("Gas cost overflow".to_string()))?;
        
        let total = gas_cost.checked_add(value)
            .ok_or_else(|| ExecutorError::Transaction("Total cost overflow".to_string()))?;
        
        Ok(total)
    }
    
    /// 执行交易
    /// 
    /// # 参数
    /// - `tx`: 交易数据
    /// - `block_env`: 区块环境
    /// 
    /// # 返回
    /// 执行结果
    pub fn execute(
        &mut self,
        tx: &Transaction,
        block_env: BlockEnv,
    ) -> Result<ExecutionResult, ExecutorError> {
        // 1. 构建交易环境
        let tx_env = self.build_tx_env(tx)?;
        
        // 2. 委托给 RevmAdapter 执行
        self.adapter.execute(tx_env, block_env)
    }
    
    /// 构建交易环境
    fn build_tx_env(&self, tx: &Transaction) -> Result<TxEnv, ExecutorError> {
        let mut tx_env = TxEnv::default();
        
        // 解析字段
        tx_env.caller = tx.from_address()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        tx_env.transact_to = match tx.to_address()? {
            Some(addr) => TransactTo::Call(addr),
            None => TransactTo::Create,
        };
        
        tx_env.value = tx.value_wei()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        tx_env.data = tx.data_bytes()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        tx_env.gas_limit = tx.gas_limit()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        tx_env.nonce = Some(tx.nonce_value()
            .map_err(|e| ExecutorError::Transaction(e))?);
        
        // Gas price（可选）
        if let Some(ref gas_price_str) = tx.gas_price {
            let hex = gas_price_str.trim_start_matches("0x");
            tx_env.gas_price = U256::from_str_radix(hex, 16)
                .map_err(|e| ExecutorError::Transaction(format!("Invalid gas_price: {}", e)))?;
        }
        
        // Chain ID（可选）
        if let Some(chain_id) = tx.chain_id {
            tx_env.chain_id = Some(chain_id);
        }
        
        Ok(tx_env)
    }
    
    /// 获取数据库的可变引用
    pub fn db_mut(&mut self) -> &mut RedbStateDB {
        self.adapter.db_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Account;
    use crate::db::StateDatabase;
    use alloy_primitives::address;
    use tempfile::TempDir;
    
    fn create_test_db() -> (RedbStateDB, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        (db, temp_dir)
    }
    
    #[test]
    fn test_simple_transfer() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置发送方账户
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let _to = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
        
        let mut from_account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        from_account.nonce = 0;
        db.set_account(&from, from_account).unwrap();
        
        // 构建交易
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0xde0b6b3a7640000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas: "0x5208".to_string(), // 21000
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let mut executor = TransactionExecutor::new(db);
        let block_env = BlockEnv::default();
        
        // 开始事务
        executor.db_mut().begin_transaction().unwrap();
        
        let result = executor.execute(&tx, block_env).unwrap();
        
        println!("Execution result: {:?}", result);
        assert!(result.gas_used > 0);
        assert!(result.success);
    }
    
    #[test]
    fn test_validate_transaction_zero_gas() {
        let (db, _temp_dir) = create_test_db();
        let mut executor = TransactionExecutor::new(db);
        
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x0".to_string(), // Gas 为 0
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let result = executor.validate_transaction(&tx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExecutorError::InvalidGas));
    }
    
    #[test]
    fn test_validate_transaction_nonce_too_low() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置账户 nonce 为 5
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
        account.nonce = 5;
        db.set_account(&from, account).unwrap();
        
        let mut executor = TransactionExecutor::new(db);
        
        // 交易 nonce 为 3，低于账户 nonce
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x3".to_string(), // nonce = 3
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let result = executor.validate_transaction(&tx);
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutorError::NonceTooLow { expected, got } => {
                assert_eq!(expected, 5);
                assert_eq!(got, 3);
            }
            _ => panic!("Expected NonceTooLow error"),
        }
    }
    
    #[test]
    fn test_validate_transaction_insufficient_balance() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置账户余额为 0.1 ETH
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let account = Account::with_balance(U256::from(100_000_000_000_000_000u64)); // 0.1 ETH
        db.set_account(&from, account).unwrap();
        
        let mut executor = TransactionExecutor::new(db);
        
        // 尝试转账 1 ETH
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0xde0b6b3a7640000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let result = executor.validate_transaction(&tx);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutorError::InsufficientFunds { .. }
        ));
    }
    
    #[test]
    fn test_validate_transaction_success() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置账户余额充足
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        account.nonce = 0;
        db.set_account(&from, account).unwrap();
        
        let mut executor = TransactionExecutor::new(db);
        
        let tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0xde0b6b3a7640000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(),
            hash: None,
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let result = executor.validate_transaction(&tx);
        assert!(result.is_ok());
    }
}
