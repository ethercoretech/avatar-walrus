//! 区块执行器
//! 
//! 批量执行区块中的所有交易

use alloy_primitives::B256;
use revm::primitives::BlockEnv;
use std::collections::HashMap;
use crate::db::{RedbStateDB, StateDatabase};
use crate::executor::{ExecutorError, TransactionExecutor, ExecutionResult};
use crate::schema::{Block, TransactionReceipt};
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
    pub fn new(db: RedbStateDB) -> Self {
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
                    
            // 预验证交易
            if let Err(e) = self.tx_executor.validate_transaction(tx) {
                failed_txs += 1;
                tracing::warn!("交易验证失败 [{}]: {}", tx_hash, e);
                continue; // 跳过该交易,不影响其他交易
            }
                    
            // 执行交易
            match self.tx_executor.execute(tx, block_env.clone()) {
                Ok(result) => {
                    total_gas_used += result.gas_used;
                            
                    if result.success {
                        successful_txs += 1;
                    } else {
                        failed_txs += 1;
                    }
                            
                    // 构建交易收据
                    use crate::executor::receipts::ReceiptBuilder;
                    
                    // 将 tx_hash 字符串转换为 B256
                    let tx_hash_b256 = if tx_hash.starts_with("0x") && tx_hash.len() == 66 {
                        // 有效的十六进制哈希
                        hex::decode(tx_hash.trim_start_matches("0x"))
                            .ok()
                            .and_then(|bytes| {
                                if bytes.len() == 32 {
                                    Some(B256::from_slice(&bytes))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| {
                                // 解码失败，使用确定性哈希
                                use sha2::{Digest, Sha256};
                                let mut hasher = Sha256::new();
                                hasher.update(tx_hash.as_bytes());
                                B256::from_slice(&hasher.finalize())
                            })
                    } else {
                        // 不是有效哈希（如 "tx_0"），使用确定性哈希
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(tx_hash.as_bytes());
                        B256::from_slice(&hasher.finalize())
                    };
                    
                    let receipt = ReceiptBuilder::build(
                        tx_hash_b256,
                        index as u64,
                        block,
                        tx,
                        &result,
                        total_gas_used,
                    );
                    receipts.insert(tx_hash.clone(), receipt);
                            
                    execution_results.insert(tx_hash, result);
                }
                Err(e) => {
                    // 执行失败,根据错误类型处理
                    if e.is_fatal() {
                        // 严重错误,回滚整个区块事务
                        self.tx_executor.db_mut().rollback_transaction()
                            .map_err(|e| ExecutorError::Database(e.to_string()))?;
                                
                        return Err(e);
                    } else {
                        // 非严重错误,跳过该交易
                        failed_txs += 1;
                        tracing::warn!("交易执行失败 [{}]: {}", tx_hash, e);
                    }
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
        use crate::trie::StateRootCalculator;
        
        let calculator = StateRootCalculator::new(self.tx_executor.db());
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
    pub fn db_mut(&mut self) -> &mut RedbStateDB {
        self.tx_executor.db_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::BlockHeader;
    use chrono::Utc;
    use tempfile::TempDir;
    
    fn create_test_db() -> (RedbStateDB, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        (db, temp_dir)
    }
    
    #[tokio::test]
    async fn test_block_execution() {
        let (db, _temp_dir) = create_test_db();
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
    
    #[tokio::test]
    async fn test_block_execution_with_invalid_tx() {
        let (mut db, _temp_dir) = create_test_db();
        
        // 设置账户余额
        use alloy_primitives::{address, U256};
        use crate::schema::{Account, Transaction};
        
        let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        account.nonce = 0;
        db.set_account(&from, account).unwrap();
        
        let mut executor = BlockExecutor::new(db);
        
        // 构建区块,包含一笔有效交易和一笔无效交易
        let valid_tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0xde0b6b3a7640000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(),
            hash: Some("0xvalid".to_string()),
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let invalid_tx = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x0".to_string(), // Gas 为 0，无效
            nonce: "0x1".to_string(),
            hash: Some("0xinvalid".to_string()),
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let block = Block {
            header: BlockHeader {
                number: 1,
                parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                timestamp: Utc::now(),
                tx_count: 2,
                transactions_root: "0x".to_string(),
                state_root: None,
                gas_used: None,
                gas_limit: Some(30_000_000),
                receipts_root: None,
            },
            transactions: vec![valid_tx, invalid_tx],
        };
        
        let result = executor.execute_block(&block).await.unwrap();
        
        // 有效交易应该执行成功,无效交易应该被跳过
        assert_eq!(result.successful_txs, 1);
        assert_eq!(result.failed_txs, 1);
        assert!(result.total_gas_used > 0);
    }
    
    #[tokio::test]
    async fn test_block_execution_partial_failure() {
        let (mut db, _temp_dir) = create_test_db();
        
        use alloy_primitives::{address, U256};
        use crate::schema::{Account, Transaction};
        
        // 设置两个账户
        let from1 = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let from2 = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
        
        let mut account1 = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        account1.nonce = 0;
        db.set_account(&from1, account1).unwrap();
        
        let mut account2 = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
        account2.nonce = 0;
        db.set_account(&from2, account2).unwrap();
        
        let mut executor = BlockExecutor::new(db);
        
        // 构建区块:
        // - 第1笔: from1的有效交易
        // - 第2笔: from2的无效交易 (Gas为0,应被跳过)
        // - 第3笔: from2的有效交易
        let tx1 = Transaction {
            from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            to: Some("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb0".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(),
            hash: Some("0xtx1".to_string()),
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let tx2 = Transaction {
            from: "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string(),
            to: Some("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb0".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x0".to_string(), // Gas为0,在验证阶段被拒绝
            nonce: "0x0".to_string(),
            hash: Some("0xtx2".to_string()),
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let tx3 = Transaction {
            from: "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string(),
            to: Some("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb0".to_string()),
            value: "0x0".to_string(),
            data: "0x".to_string(),
            gas: "0x5208".to_string(),
            nonce: "0x0".to_string(), // 使用 from2 的第一笔交易
            hash: Some("0xtx3".to_string()),
            gas_price: Some("0x3b9aca00".to_string()),
            chain_id: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };
        
        let block = Block {
            header: BlockHeader {
                number: 1,
                parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                timestamp: Utc::now(),
                tx_count: 3,
                transactions_root: "0x".to_string(),
                state_root: None,
                gas_used: None,
                gas_limit: Some(30_000_000),
                receipts_root: None,
            },
            transactions: vec![tx1, tx2, tx3],
        };
        
        let result = executor.execute_block(&block).await.unwrap();
        
        // 第1和第3笔应该成功,第2笔应该失败
        assert_eq!(result.successful_txs, 2);
        assert_eq!(result.failed_txs, 1);
    }
}
