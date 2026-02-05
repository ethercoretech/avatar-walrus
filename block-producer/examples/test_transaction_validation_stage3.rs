//! 交易验证集成测试
//! 
//! 测试交易验证功能的完整流程

use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::executor::{block_executor::BlockExecutor, TransactionExecutor, ExecutorError};
use block_producer::schema::{Account, Transaction, Block, BlockHeader};
use alloy_primitives::{address, U256};
use chrono::Utc;

fn main() {
    println!("🧪 开始交易验证集成测试\n");
    
    test_invalid_gas();
    test_nonce_too_low();
    test_insufficient_balance();
    test_valid_transaction();
    test_block_execution();
    
    println!("\n🎉 所有测试完成!");
    println!("✅ 交易验证功能正常工作");
}

fn test_invalid_gas() {
    println!("📋 测试 1: Gas 为 0 的交易（应被拒绝）");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test1.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
    account.nonce = 0;
    db.set_account(&from, account).unwrap();
    
    let mut executor = TransactionExecutor::new(db);
    
    let invalid_gas_tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
        value: "0x0".to_string(),
        data: "0x".to_string(),
        gas: "0x0".to_string(),
        nonce: "0x0".to_string(),
        hash: Some("0xinvalid_gas".to_string()),
        gas_price: Some("0x3b9aca00".to_string()),
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    
    match executor.validate_transaction(&invalid_gas_tx) {
        Ok(_) => println!("   ❌ 测试失败: 应该拒绝 Gas 为 0 的交易"),
        Err(ExecutorError::InvalidGas) => println!("   ✓ 正确拒绝: InvalidGas"),
        Err(e) => println!("   ❌ 错误类型不符: {:?}", e),
    }
    println!();
}

fn test_nonce_too_low() {
    println!("📋 测试 2: Nonce 过低的交易（应被拒绝）");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test2.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
    account.nonce = 5;
    db.set_account(&from, account).unwrap();
    
    let mut executor = TransactionExecutor::new(db);
    
    let low_nonce_tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
        value: "0x0".to_string(),
        data: "0x".to_string(),
        gas: "0x5208".to_string(),
        nonce: "0x2".to_string(),
        hash: Some("0xlow_nonce".to_string()),
        gas_price: Some("0x3b9aca00".to_string()),
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    
    match executor.validate_transaction(&low_nonce_tx) {
        Ok(_) => println!("   ❌ 测试失败: 应该拒绝 Nonce 过低的交易"),
        Err(ExecutorError::NonceTooLow { expected, got }) => {
            println!("   ✓ 正确拒绝: NonceTooLow");
            println!("     期望 nonce: {}, 实际: {}", expected, got);
        }
        Err(e) => println!("   ❌ 错误类型不符: {:?}", e),
    }
    println!();
}

fn test_insufficient_balance() {
    println!("📋 测试 3: 余额不足的交易（应被拒绝）");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test3.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    let account = Account::with_balance(U256::from(100_000_000_000_000_000u64)); // 0.1 ETH
    db.set_account(&from, account).unwrap();
    
    let mut executor = TransactionExecutor::new(db);
    
    let insufficient_balance_tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
        value: "0xde0b6b3a7640000".to_string(), // 1 ETH
        data: "0x".to_string(),
        gas: "0x5208".to_string(),
        nonce: "0x0".to_string(),
        hash: Some("0xinsufficient".to_string()),
        gas_price: Some("0x3b9aca00".to_string()),
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    
    match executor.validate_transaction(&insufficient_balance_tx) {
        Ok(_) => println!("   ❌ 测试失败: 应该拒绝余额不足的交易"),
        Err(ExecutorError::InsufficientFunds { required, available }) => {
            println!("   ✓ 正确拒绝: InsufficientFunds");
            println!("     所需: {} wei", required);
            println!("     可用: {} wei", available);
        }
        Err(e) => println!("   ❌ 错误类型不符: {:?}", e),
    }
    println!();
}

fn test_valid_transaction() {
    println!("📋 测试 4: 有效交易（应通过验证）");
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test4.redb");
    let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    
    let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    let mut account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64)); // 10 ETH
    account.nonce = 0;
    db.set_account(&from, account).unwrap();
    
    let mut executor = TransactionExecutor::new(db);
    
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
    
    match executor.validate_transaction(&valid_tx) {
        Ok(_) => println!("   ✓ 验证通过"),
        Err(e) => println!("   ❌ 测试失败: 应该通过验证，但得到错误: {:?}", e),
    }
    println!();
}

fn test_block_execution() {
    println!("📋 测试 5: 区块执行（包含有效和无效交易）");
    
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_block.redb");
        let mut db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
        
        // 设置两个账户
        let from1 = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        let from2 = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
        
        let mut account1 = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
        account1.nonce = 0;
        db.set_account(&from1, account1).unwrap();
        
        let mut account2 = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
        account2.nonce = 0;
        db.set_account(&from2, account2).unwrap();
        
        let mut block_executor = BlockExecutor::new(db);
        
        // 构建包含3笔交易的区块: from1有效, from2无效(Gas为0), from2有效
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
            gas: "0x0".to_string(), // Gas 为 0, 无效
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
            nonce: "0x0".to_string(), // 使用 from2 的有效交易
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
        
        let result = block_executor.execute_block(&block).await.unwrap();
        
        println!("   区块执行结果:");
        println!("   ✓ 成功交易: {}", result.successful_txs);
        println!("   ✓ 失败交易: {}", result.failed_txs);
        println!("   ✓ 总 Gas 使用: {}", result.total_gas_used);
        println!("   ✓ 执行结果数: {}", result.execution_results.len());
        
        // 验证结果
        assert_eq!(result.successful_txs, 2, "应该有2笔成功交易");
        assert_eq!(result.failed_txs, 1, "应该有1笔失败交易");
        assert!(result.total_gas_used > 0, "总 Gas 应该大于 0");
        
        println!("\n   ✓ 区块执行测试通过!");
    });
}
