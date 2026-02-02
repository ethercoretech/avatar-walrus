use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::schema::Account;
use alloy_primitives::{Address, U256};
use tempfile::TempDir;

fn create_test_db() -> (RedbStateDB, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.redb");
    let db = RedbStateDB::new(db_path.to_str().unwrap()).unwrap();
    (db, temp_dir)
}

#[test]
fn test_redb_account_crud() {
    let (mut db, _temp_dir) = create_test_db();
    
    let addr = Address::random();
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
    
    println!("✅ Account CRUD test passed");
}

#[test]
fn test_redb_storage_crud() {
    let (mut db, _temp_dir) = create_test_db();
    
    let addr = Address::random();
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
    
    println!("✅ Storage CRUD test passed");
}

#[test]
fn test_redb_transaction() {
    let (mut db, _temp_dir) = create_test_db();
    
    let addr = Address::random();
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
    
    println!("✅ Transaction test passed");
}

#[test]
fn test_redb_transaction_rollback() {
    let (mut db, _temp_dir) = create_test_db();
    
    let addr = Address::random();
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
    
    println!("✅ Transaction rollback test passed");
}

#[test]
fn test_redb_changed_accounts_tracking() {
    let (mut db, _temp_dir) = create_test_db();
    
    let addr1 = Address::random();
    let addr2 = Address::random();
    
    db.begin_transaction().unwrap();
    
    db.set_account(&addr1, Account::with_balance(U256::from(100))).unwrap();
    db.set_account(&addr2, Account::with_balance(U256::from(200))).unwrap();
    
    let changed = db.get_changed_accounts().unwrap();
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&addr1));
    assert!(changed.contains(&addr2));
    
    db.commit_transaction().unwrap();
    
    println!("✅ Changed accounts tracking test passed");
}
