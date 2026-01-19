//! 交易执行器
//! 
//! 执行单笔交易并返回结果

use alloy_primitives::{Address, U256, B256, Bytes};
use revm::{EVM, primitives::{TxEnv, BlockEnv, ExecutionResult as RevmExecutionResult, Output}};
use crate::db::WalrusStateDB;
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
    pub fn new(db: WalrusStateDB) -> Self {
        Self {
            adapter: RevmAdapter::new(db),
        }
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
        
        // 2. 创建 EVM 实例
        let mut evm = EVM::builder()
            .with_db(&mut self.adapter)
            .with_block_env(block_env)
            .with_tx_env(tx_env)
            .build();
        
        // 3. 执行交易
        let result = evm.transact()
            .map_err(|e| ExecutorError::Evm(format!("{:?}", e)))?;
        
        // 4. 处理执行结果
        self.process_result(result, tx)
    }
    
    /// 构建交易环境
    fn build_tx_env(&self, tx: &Transaction) -> Result<TxEnv, ExecutorError> {
        let mut tx_env = TxEnv::default();
        
        // 解析字段
        tx_env.caller = tx.from_address()
            .map_err(|e| ExecutorError::Transaction(e))?;
        
        tx_env.transact_to = match tx.to_address()? {
            Some(addr) => revm::primitives::TransactTo::Call(addr),
            None => revm::primitives::TransactTo::Create,
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
    
    /// 处理执行结果
    fn process_result(
        &self,
        result: RevmExecutionResult,
        tx: &Transaction,
    ) -> Result<ExecutionResult, ExecutorError> {
        let success = result.is_success();
        let gas_used = result.gas_used();
        
        let (output, contract_address) = match result {
            RevmExecutionResult::Success { output, gas_refunded, logs, .. } => {
                let (out, contract_addr) = match output {
                    Output::Call(data) => (Some(data), None),
                    Output::Create(data, addr) => (Some(data), addr),
                };
                
                return Ok(ExecutionResult {
                    gas_used,
                    success: true,
                    output: out,
                    contract_address: contract_addr,
                    gas_refund: gas_refunded,
                    logs,
                });
            }
            RevmExecutionResult::Revert { output, gas_used } => {
                (Some(output), None)
            }
            RevmExecutionResult::Halt { reason, gas_used } => {
                return Err(ExecutorError::Evm(format!("Halted: {:?}", reason)));
            }
        };
        
        Ok(ExecutionResult {
            gas_used,
            success,
            output,
            contract_address,
            gas_refund: 0,
            logs: Vec::new(),
        })
    }
    
    /// 获取数据库的可变引用
    pub fn db_mut(&mut self) -> &mut WalrusStateDB {
        self.adapter.db_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Account;
    use alloy_primitives::address;
    
    #[test]
    fn test_simple_transfer() {
        let mut db = WalrusStateDB::new().unwrap();
        
        // 设置发送方账户
        let from = address!("742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let to = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
        
        let mut from_account = Account::with_balance(U256::from(1_000_000_000_000_000_000u64));
        from_account.nonce = 0;
        db.set_account(&from, from_account).unwrap();
        
        // 构建交易
        let tx = Transaction {
            from: "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
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
    }
}
