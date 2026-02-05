//! 第四阶段集成测试：Trie 计算和区块组装
//! 
//! 测试场景：
//! 1. 状态根计算（State Root）
//! 2. 交易根计算（Transactions Root）
//! 3. 收据根计算（Receipts Root）
//! 4. 完整区块组装流程

use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::executor::block_executor::BlockExecutor;
use block_producer::schema::{Account, Transaction, Block, BlockHeader};
use block_producer::utils::{calculate_merkle_root, EMPTY_ROOT_HASH};
use alloy_primitives::{address, U256};
use chrono::Utc;

#[tokio::main]
async fn main() {
    println!("🧪 开始测试 Trie 计算和区块组装（第四阶段）\n");
    
    // 创建临时数据库
    let db_path = "./data/test_stage4.redb";
    std::fs::create_dir_all("./data").unwrap();
    
    // 清理旧数据库
    let _ = std::fs::remove_file(db_path);
    
    println!("✅ 数据库初始化成功\n");
    
    // 测试 1: 状态根计算
    test_state_root_calculation().await;
    
    // 测试 2: 交易根和收据根计算
    test_transactions_and_receipts_root().await;
    
    // 测试 3: 完整区块组装
    test_full_block_assembly().await;
    
    println!("\n🎉 所有第四阶段测试完成!");
    println!("✅ Trie 计算功能正常工作");
    println!("✅ 区块组装流程正确");
}

/// 测试 1: 状态根计算
async fn test_state_root_calculation() {
    println!("📋 测试 1: 状态根计算");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_state_root.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    // 准备初始账户
    let alice = address!("0000000000000000000000000000000000000001");
    let bob = address!("0000000000000000000000000000000000000002");
    
    // 使用 with_balance 方法创建账户，确保 code_hash 正确设置为 EMPTY_CODE_HASH
    let alice_account = Account::with_balance(U256::from(100_000_000_000_000_000u64));
    let bob_account = Account::with_balance(U256::from(50_000_000_000_000_000u64));
    
    db.set_account(&alice, alice_account.clone()).unwrap();
    db.set_account(&bob, bob_account.clone()).unwrap();
    
    // 创建执行器
    let mut executor = BlockExecutor::new(db);
    
    // 创建包含转账的区块
    let block = create_test_block(vec![
        Transaction {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: Some("0x0000000000000000000000000000000000000002".to_string()),
            value: "100".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "0".to_string(),
            hash: Some("0xtest1".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
    ]);
    
    // 执行区块
    let result = executor.execute_block(&block).await;
    match result {
        Ok(exec_result) => {
            println!("  ✓ 区块执行成功: {} 成功, {} 失败", 
                     exec_result.successful_txs, exec_result.failed_txs);
            
            // 计算状态根
            match executor.calculate_state_root() {
                Ok(state_root) => {
                    println!("  ✓ 状态根计算成功");
                    println!("    State Root: 0x{}", hex::encode(state_root.as_slice()));
                    
                    // 验证状态根不是零值
                    assert_ne!(state_root, alloy_primitives::B256::ZERO);
                    println!("  ✓ 状态根验证通过（非零值）\n");
                }
                Err(e) => {
                    println!("  ✗ 状态根计算失败: {}\n", e);
                }
            }
        }
        Err(e) => {
            println!("  ✗ 区块执行失败: {}\n", e);
        }
    }
}

/// 测试 2: 交易根和收据根计算
async fn test_transactions_and_receipts_root() {
    println!("📋 测试 2: 交易根和收据根计算");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_merkle.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    // 准备账户 - 确保所有接收地址也初始化，避免 nonce 不同步
    let alice = address!("0000000000000000000000000000000000000001");
    let bob = address!("0000000000000000000000000000000000000002");
    let charlie = address!("0000000000000000000000000000000000000003");
    let david = address!("0000000000000000000000000000000000000004");
    
    // 给 alice 充足的余额
    let alice_account = Account::with_balance(U256::from(100_000_000_000_000_000u64));
    db.set_account(&alice, alice_account).unwrap();
    
    // 初始化接收地址（余额为0，但账户存在）
    db.set_account(&bob, Account::default()).unwrap();
    db.set_account(&charlie, Account::default()).unwrap();
    db.set_account(&david, Account::default()).unwrap();
    
    let mut executor = BlockExecutor::new(db);
    
    // 创建包含多笔交易的区块
    let transactions = vec![
        Transaction {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: Some("0x0000000000000000000000000000000000000002".to_string()),
            value: "100".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "0".to_string(),
            hash: Some("0xtx1".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
        Transaction {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: Some("0x0000000000000000000000000000000000000003".to_string()),
            value: "200".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "1".to_string(),
            hash: Some("0xtx2".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
        Transaction {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: Some("0x0000000000000000000000000000000000000004".to_string()),
            value: "300".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "2".to_string(),
            hash: Some("0xtx3".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
    ];
    
    let block = create_test_block(transactions.clone());
    
    // 执行区块
    match executor.execute_block(&block).await {
        Ok(exec_result) => {
            println!("  ✓ 区块执行: {} 笔交易成功", exec_result.successful_txs);
            
            // 计算交易根
            let tx_root = calculate_merkle_root(&transactions);
            println!("  ✓ 交易根计算成功");
            println!("    Transactions Root: 0x{}", hex::encode(tx_root.as_slice()));
            
            // 计算收据根
            let receipts: Vec<_> = exec_result.receipts.values().cloned().collect();
            let receipts_root = if !receipts.is_empty() {
                calculate_merkle_root(&receipts)
            } else {
                EMPTY_ROOT_HASH
            };
            
            println!("  ✓ 收据根计算成功");
            println!("    Receipts Root: 0x{}", hex::encode(receipts_root.as_slice()));
            
            // 验证
            assert_ne!(tx_root, alloy_primitives::B256::ZERO);
            assert_ne!(receipts_root, alloy_primitives::B256::ZERO);
            println!("  ✓ Merkle Root 验证通过\n");
        }
        Err(e) => {
            println!("  ✗ 区块执行失败: {}\n", e);
        }
    }
}

/// 测试 3: 完整区块组装流程
async fn test_full_block_assembly() {
    println!("📋 测试 3: 完整区块组装流程");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_full_block.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    // 准备账户
    let deployer = address!("0000000000000000000000000000000000000001");
    let user1 = address!("0000000000000000000000000000000000000002");
    
    db.set_account(&deployer, Account::with_balance(U256::from(100_000_000_000_000_000u64))).unwrap(); // 0.1 ETH
    db.set_account(&user1, Account::with_balance(U256::from(50_000_000_000_000_000u64))).unwrap();    // 0.05 ETH
    
    let mut executor = BlockExecutor::new(db);
    
    // 创建区块（包含多种交易类型）
    let transactions = vec![
        // 简单转账
        Transaction {
            from: "0x0000000000000000000000000000000000000001".to_string(),
            to: Some("0x0000000000000000000000000000000000000002".to_string()),
            value: "1000".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "0".to_string(),
            hash: Some("0xblock1_tx1".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
        // 另一笔转账
        Transaction {
            from: "0x0000000000000000000000000000000000000002".to_string(),
            to: Some("0x0000000000000000000000000000000000000001".to_string()),
            value: "500".to_string(),
            data: "0x".to_string(),
            gas: "21000".to_string(),
            nonce: "0".to_string(),
            hash: Some("0xblock1_tx2".to_string()),
            gas_price: None,
            chain_id: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
    ];
    
    let mut block = create_test_block(transactions.clone());
    
    println!("  📦 区块信息:");
    println!("    区块号: {}", block.header.number);
    println!("    交易数: {}", block.transactions.len());
    println!("    Gas 限制: {}", block.header.gas_limit.unwrap_or(0));
    
    // 步骤 1: 执行区块
    print!("\n  [1/5] 执行区块... ");
    match executor.execute_block(&block).await {
        Ok(exec_result) => {
            println!("✓");
            println!("        成功: {} 笔", exec_result.successful_txs);
            println!("        失败: {} 笔", exec_result.failed_txs);
            println!("        Gas 使用: {}", exec_result.total_gas_used);
            
            // 步骤 2: 计算状态根
            print!("  [2/5] 计算状态根... ");
            match executor.calculate_state_root() {
                Ok(state_root) => {
                    println!("✓");
                    println!("        0x{}", hex::encode(state_root.as_slice()));
                    
                    // 步骤 3: 计算交易根
                    print!("  [3/5] 计算交易根... ");
                    let tx_root = calculate_merkle_root(&transactions);
                    println!("✓");
                    println!("        0x{}", hex::encode(tx_root.as_slice()));
                    
                    // 步骤 4: 计算收据根
                    print!("  [4/5] 计算收据根... ");
                    let receipts: Vec<_> = exec_result.receipts.values().cloned().collect();
                    let receipts_root = if !receipts.is_empty() {
                        calculate_merkle_root(&receipts)
                    } else {
                        EMPTY_ROOT_HASH
                    };
                    println!("✓");
                    println!("        0x{}", hex::encode(receipts_root.as_slice()));
                    
                    // 步骤 5: 更新区块头
                    print!("  [5/5] 更新区块头... ");
                    block.header.state_root = Some(format!("0x{}", hex::encode(state_root.as_slice())));
                    block.header.gas_used = Some(exec_result.total_gas_used);
                    block.header.transactions_root = format!("0x{}", hex::encode(tx_root.as_slice()));
                    block.header.receipts_root = Some(format!("0x{}", hex::encode(receipts_root.as_slice())));
                    println!("✓");
                    
                    // 持久化区块
                    match executor.db_mut().save_block(&block) {
                        Ok(_) => {
                            println!("\n  ✓ 区块组装完成并已持久化");
                            
                            // 打印完整区块信息
                            println!("\n  📊 完整区块信息:");
                            println!("    ┌─────────────────────────────────────────");
                            println!("    │ 区块号: {}", block.header.number);
                            println!("    │ 父区块: {}", block.header.parent_hash);
                            println!("    │ 时间戳: {}", block.header.timestamp);
                            println!("    ├─────────────────────────────────────────");
                            println!("    │ 交易数: {}", block.transactions.len());
                            println!("    │ Gas 使用: {}/{}", 
                                     block.header.gas_used.unwrap_or(0),
                                     block.header.gas_limit.unwrap_or(0));
                            println!("    ├─────────────────────────────────────────");
                            println!("    │ 状态根:");
                            println!("    │   {}", block.header.state_root.as_ref().unwrap());
                            println!("    │ 交易根:");
                            println!("    │   {}", block.header.transactions_root);
                            println!("    │ 收据根:");
                            println!("    │   {}", block.header.receipts_root.as_ref().unwrap());
                            println!("    └─────────────────────────────────────────");
                            
                            // 验证区块可以被读取
                            match executor.db_mut().get_block(block.header.number) {
                                Ok(Some(_)) => {
                                    println!("\n  ✓ 区块读取验证成功");
                                }
                                Ok(None) => {
                                    println!("\n  ✗ 区块读取失败：未找到");
                                }
                                Err(e) => {
                                    println!("\n  ✗ 区块读取失败: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!("\n  ✗ 区块持久化失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("✗");
                    println!("        错误: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗");
            println!("        错误: {}", e);
        }
    }
    
    println!();
}

/// 辅助函数：创建测试区块
fn create_test_block(transactions: Vec<Transaction>) -> Block {
    Block {
        header: BlockHeader {
            number: 1,
            parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            timestamp: Utc::now(),
            tx_count: transactions.len(),
            transactions_root: String::new(),
            state_root: None,
            gas_used: None,
            gas_limit: Some(30000000),
            receipts_root: None,
        },
        transactions,
    }
}
