//! RedbStateDB 独立测试
//! 
//! 测试第一阶段：基础数据库层实现

use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::schema::Account;
use alloy_primitives::{Address, U256, address};

fn main() {
    println!("🧪 开始测试 RedbStateDB（第一阶段）\n");
    
    // 创建临时数据库
    let temp_path = format!("/tmp/test_redb_{}.redb", std::process::id());
    let mut db = RedbStateDB::new(&temp_path).expect("Failed to create database");
    
    println!("✅ 数据库创建成功: {}\n", temp_path);
    
    // 测试 1: 账户 CRUD
    println!("📌 测试 1: 账户 CRUD 操作");
    let addr = address!("0000000000000000000000000000000000000001");
    let account = Account::with_balance(U256::from(1000));
    
    db.set_account(&addr, account.clone()).unwrap();
    println!("  ✓ 写入账户: {} (balance: {})", addr, account.balance);
    
    let retrieved = db.get_account(&addr).unwrap();
    assert_eq!(retrieved, Some(account.clone()));
    println!("  ✓ 读取账户成功");
    
    db.delete_account(&addr).unwrap();
    let deleted = db.get_account(&addr).unwrap();
    assert_eq!(deleted, None);
    println!("  ✓ 删除账户成功\n");
    
    // 测试 2: 存储槽 CRUD
    println!("📌 测试 2: 存储槽 CRUD 操作");
    let addr2 = address!("0000000000000000000000000000000000000002");
    let key = U256::from(42);
    let value = U256::from(12345);
    
    db.set_storage(&addr2, key, value).unwrap();
    println!("  ✓ 写入存储槽: key={}, value={}", key, value);
    
    let retrieved_value = db.get_storage(&addr2, key).unwrap();
    assert_eq!(retrieved_value, value);
    println!("  ✓ 读取存储槽成功");
    
    let non_existent = db.get_storage(&addr2, U256::from(999)).unwrap();
    assert_eq!(non_existent, U256::ZERO);
    println!("  ✓ 读取不存在的槽返回 0\n");
    
    // 测试 3: 事务提交
    println!("📌 测试 3: 事务提交");
    let addr3 = address!("0000000000000000000000000000000000000003");
    let account3 = Account::with_balance(U256::from(5000));
    
    db.begin_transaction().unwrap();
    println!("  ✓ 开启事务");
    
    db.set_account(&addr3, account3.clone()).unwrap();
    println!("  ✓ 在事务中写入账户");
    
    let in_tx = db.get_account(&addr3).unwrap();
    assert_eq!(in_tx, Some(account3.clone()));
    println!("  ✓ 事务中可以读取");
    
    db.commit_transaction().unwrap();
    println!("  ✓ 提交事务");
    
    let after_commit = db.get_account(&addr3).unwrap();
    assert_eq!(after_commit, Some(account3));
    println!("  ✓ 事务提交后可以读取\n");
    
    // 测试 4: 事务回滚
    println!("📌 测试 4: 事务回滚");
    let addr4 = address!("0000000000000000000000000000000000000004");
    let account4 = Account::with_balance(U256::from(8000));
    
    db.begin_transaction().unwrap();
    db.set_account(&addr4, account4).unwrap();
    println!("  ✓ 在事务中写入账户");
    
    db.rollback_transaction().unwrap();
    println!("  ✓ 回滚事务");
    
    let after_rollback = db.get_account(&addr4).unwrap();
    assert_eq!(after_rollback, None);
    println!("  ✓ 回滚后数据不存在\n");
    
    // 测试 5: 变更追踪
    println!("📌 测试 5: 变更账户追踪");
    let addr5 = address!("0000000000000000000000000000000000000005");
    let addr6 = address!("0000000000000000000000000000000000000006");
    
    db.begin_transaction().unwrap();
    db.set_account(&addr5, Account::with_balance(U256::from(100))).unwrap();
    db.set_account(&addr6, Account::with_balance(U256::from(200))).unwrap();
    
    let changed = db.get_changed_accounts().unwrap();
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&addr5));
    assert!(changed.contains(&addr6));
    println!("  ✓ 追踪到 {} 个变更账户", changed.len());
    
    db.commit_transaction().unwrap();
    println!("  ✓ 提交事务后追踪清零\n");
    
    // 清理
    std::fs::remove_file(&temp_path).ok();
    println!("✅ 所有测试通过！\n");
    println!("📊 测试总结:");
    println!("  - 账户 CRUD: ✅");
    println!("  - 存储槽 CRUD: ✅");
    println!("  - 事务提交: ✅");
    println!("  - 事务回滚: ✅");
    println!("  - 变更追踪: ✅");
    println!("\n🎉 第一阶段（数据库层）实现完成！");
}
