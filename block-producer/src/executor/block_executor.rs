//! 区块执行器
//! 
//! 批量执行区块中的所有交易

use alloy_primitives::{B256, Address};
use revm::primitives::BlockEnv;
use std::collections::HashMap;
use crate::db::WalrusStateDB;
use crate::executor::{ExecutorError, TransactionExecutor, ExecutionResult};
use crate::schema::{Block, TransactionReceipt};
use crate::trie::StateRootCalculator;
use serde::{Deserialize, Serialize};

/// 区块执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecutionResult {
    /// 交易哈希 -> 执行结果
    pub execution_results: HashMap<String, ExecutionResult>,
    
    /// 交易收据
    pub receipts: HashMap<String, TransactionReceipt>,
    
    /// 总 Gas 使用量
    pub total_gas_used: u64,
    
    /// 成功交易数量
    pub successful_txs: usize,
    
    /// 失败交易数量
    pub failed_txs: usize,
}

/// 区块执行器
pub struct BlockExecutor {
    tx_executor: TransactionExecutor,
}

impl BlockExecutor {
    /// 创建区块执行器
    pub fn new(db: WalrusStateDB) -> Self {
        Self {
            tx_executor: TransactionExecutor::new(db),
        }
    }
    
    /// 执行区块
    /// 
    /// 按顺序执行区块中的所有交易
    pub async fn execute_block(
        &mut self,
        block: &Block,
    ) -> Result<BlockExecutionResult, ExecutorError> {
        let mut execution_results = HashMap::new();
        let mut receipts = HashMap::new();
        let mut total_gas_used = 0u64;
        let mut successful_txs = 0;
        let mut failed_txs = 0;
        
        // 开始事务
        self.tx_executor.db_mut().begin_transaction()
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        // 构建区块环境
        let block_env = self.build_block_env(block);
        
        // 执行每笔交易
        for (index, tx) in block.transactions.iter().enumerate() {
            let tx_hash = tx.hash.clone()
                .unwrap_or_else(|| format!("tx_{}", index));
            
            match self.tx_executor.execute(tx, block_env.clone()) {
                Ok(result) => {
                    total_gas_used += result.gas_used;
                    
                    if result.success {
                        successful_txs += 1;
                    } else {
                        failed_txs += 1;
                    }
                    
                    // TODO: 构建交易收据
                    // let receipt = self.build_receipt(
                    //     &tx_hash,
                    //     index as u64,
                    //     block,
                    //     &result,
                    //     total_gas_used,
                    // );
                    // receipts.insert(tx_hash.clone(), receipt);
                    
                    execution_results.insert(tx_hash, result);
                }
                Err(e) => {
                    // 交易执行失败，回滚事务
                    self.tx_executor.db_mut().rollback_transaction()
                        .map_err(|e| ExecutorError::Database(e.to_string()))?;
                    
                    return Err(e);
                }
            }
        }
        
        // 提交事务
        self.tx_executor.db_mut().commit_transaction()
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        Ok(BlockExecutionResult {
            execution_results,
            receipts,
            total_gas_used,
            successful_txs,
            failed_txs,
        })
    }
    
    /// 计算状态根
    /// 
    /// 在区块执行完成后计算状态根
    pub fn calculate_state_root(&self) -> Result<B256, ExecutorError> {
        let calculator = StateRootCalculator::new(self.tx_executor.adapter.db());
        calculator.calculate_incremental()
            .map_err(|e| ExecutorError::Other(format!("State root calculation failed: {}", e)))
    }
    
    /// 构建区块环境
    fn build_block_env(&self, block: &Block) -> BlockEnv {
        let mut env = BlockEnv::default();
        
        env.number = alloy_primitives::U256::from(block.header.number);
        env.timestamp = alloy_primitives::U256::from(block.header.timestamp.timestamp() as u64);
        
        // Gas 限制
        if let Some(gas_limit) = block.header.gas_limit {
            env.gas_limit = alloy_primitives::U256::from(gas_limit);
        }
        
        env
    }
    
    /// 获取数据库的可变引用
    pub fn db_mut(&mut self) -> &mut WalrusStateDB {
        self.tx_executor.db_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{BlockHeader, Transaction};
    use chrono::Utc;
    
    #[tokio::test]
    async fn test_block_execution() {
        let db = WalrusStateDB::new().unwrap();
        let mut executor = BlockExecutor::new(db);
        
        // 构建测试区块
        let block = Block {
            header: BlockHeader {
                number: 1,
                parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                timestamp: Utc::now(),
                tx_count: 0,
                transactions_root: "0x".to_string(),
                state_root: None,
                gas_used: None,
                gas_limit: Some(30_000_000),
                receipts_root: None,
            },
            transactions: vec![],
        };
        
        let result = executor.execute_block(&block).await.unwrap();
        
        assert_eq!(result.total_gas_used, 0);
        assert_eq!(result.successful_txs, 0);
    }
}
