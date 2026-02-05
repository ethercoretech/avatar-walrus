//! 第二阶段集成测试：REVM 适配器和交易执行
//! 
//! 测试场景：
//! 1. 简单转账交易
//! 2. 账户状态更新
//! 3. Gas 消耗统计
//! 4. 区块批量执行

use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::executor::TransactionExecutor;
use block_producer::schema::{Account, Transaction};
use alloy_primitives::{address, U256};
use revm::primitives::BlockEnv;

fn main() {
    println!("🧪 开始测试 REVM 适配器（第二阶段）\n");
    
    // 创建临时数据库
    let db_path = "./data/test_stage2.redb";
    std::fs::create_dir_all("./data").unwrap();
    
    // 清理旧数据库
    let _ = std::fs::remove_file(db_path);
    
    let db = RedbStateDB::new(db_path).unwrap();
    println!("✅ 数据库创建成功\n");
    
    // 测试 1: 简单转账交易
    test_simple_transfer(db);
    
    println!("\n🎉 第二阶段（REVM 适配器）测试完成！");
}

fn test_simple_transfer(mut db: RedbStateDB) {
    println!("📌 测试 1: 简单转账交易");
    
    // 设置发送方和接收方地址
    let from = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    let to = address!("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
    
    // 设置发送方账户（10 ETH 余额）
    let mut from_account = Account::with_balance(U256::from(10_000_000_000_000_000_000u64));
    from_account.nonce = 0;
    db.set_account(&from, from_account).unwrap();
    println!("   - 发送方地址: {}", from);
    println!("   - 初始余额: 10 ETH");
    println!("   - 接收方地址: {}", to);
    
    // 创建交易执行器
    let mut executor = TransactionExecutor::new(db);
    
    // 构建转账交易（1 ETH）
    let tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()),
        value: "0xde0b6b3a7640000".to_string(), // 1 ETH in hex
        data: "0x".to_string(),
        gas: "0x5208".to_string(), // 21000
        nonce: "0x0".to_string(),
        hash: Some("0x1234567890abcdef".to_string()),
        gas_price: Some("0x3b9aca00".to_string()), // 1 Gwei
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    println!("   - 转账金额: 1 ETH");
    println!("   - Gas 限制: 21000");
    
    // 设置区块环境
    let block_env = BlockEnv::default();
    
    // 开始事务
    executor.db_mut().begin_transaction().unwrap();
    
    // 执行交易
    let result = executor.execute(&tx, block_env).unwrap();
    
    // 提交事务
    executor.db_mut().commit_transaction().unwrap();
    
    // 验证结果
    assert!(result.success, "交易执行失败");
    assert_eq!(result.gas_used, 21000, "Gas 消耗不正确");
    
    println!("   ✓ 交易执行成功");
    println!("   ✓ Gas 消耗: {}", result.gas_used);
    println!("   ✓ 执行状态: {}", if result.success { "成功" } else { "失败" });
    
    // 验证账户余额变化
    let from_account_after = executor.db_mut().get_account(&from).unwrap().unwrap();
    let to_account_after = executor.db_mut().get_account(&to).unwrap().unwrap();
    
    println!("   ✓ 发送方最终余额: {} wei", from_account_after.balance);
    println!("   ✓ 接收方最终余额: {} wei", to_account_after.balance);
    println!("   ✓ 发送方 nonce: {}", from_account_after.nonce);
    
    // 预期：发送方余额 = 初始 10 ETH - 1 ETH（转账）- gas_used * gas_price
    let expected_from_balance = U256::from(10_000_000_000_000_000_000u64)
        - U256::from(1_000_000_000_000_000_000u64) // 1 ETH 转账
        - U256::from(21000u64) * U256::from(1_000_000_000u64); // Gas 费用
    
    let expected_to_balance = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
    
    assert_eq!(from_account_after.balance, expected_from_balance, "发送方余额不正确");
    assert_eq!(to_account_after.balance, expected_to_balance, "接收方余额不正确");
    assert_eq!(from_account_after.nonce, 1, "Nonce 未增加");
    
    println!("   ✓ 余额变化验证通过");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stage2_integration() {
        // 清理测试数据
        let db_path = "./data/test_stage2_unit.redb";
        let _ = std::fs::remove_file(db_path);
        
        let db = RedbStateDB::new(db_path).unwrap();
        test_simple_transfer(db);
        
        // 清理
        let _ = std::fs::remove_file(db_path);
    }
}
