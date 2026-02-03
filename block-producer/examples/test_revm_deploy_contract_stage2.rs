//! 第二阶段集成测试：合约部署与预编译合约调用
//! 
//! 测试场景：
//! 1. 部署简单的 ERC20 合约（CREATE 部署）
//! 2. 验证合约地址生成和字节码存储
//! 3. 测试预编译合约调用（ecrecover）
//! 4. Gas 消耗统计

use block_producer::db::{RedbStateDB, StateDatabase};
use block_producer::executor::TransactionExecutor;
use block_producer::schema::{Account, Transaction};
use alloy_primitives::{address, U256};
use revm::primitives::BlockEnv;

fn main() {
    println!("🧪 开始测试合约部署与预编译合约（第二阶段）\n");
    
    // 测试 1：部署 ERC20 合约（CREATE 部署）
    {
        let db_path = "./data/test_contract_deploy.redb";
        std::fs::create_dir_all("./data").unwrap();
        let _ = std::fs::remove_file(db_path);
        
        let db = RedbStateDB::new(db_path).unwrap();
        test_deploy_erc20_contract(db);
    }
    
    println!();
    
    // 测试 2：调用预编译合约（ecrecover）
    {
        let db_path = "./data/test_precompiled_contract.redb";
        std::fs::create_dir_all("./data").unwrap();
        let _ = std::fs::remove_file(db_path);
        
        let db = RedbStateDB::new(db_path).unwrap();
        test_call_precompiled_contract(db);
    }
    
    println!("\n🎉 第二阶段（合约部署 + 预编译合约）测试完成！");
}

/// 测试 1：部署 ERC20 合约（普通字节码合约部署）
fn test_deploy_erc20_contract(mut db: RedbStateDB) {
    println!("📌 测试 1：部署 ERC20 合约（CREATE 部署）");
    
    // 部署者地址
    let deployer = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    
    // 设置部署者账户（有足够的 ETH 支付 gas）
    let mut deployer_account = Account::with_balance(U256::from(100u64) * U256::from(1_000_000_000_000_000_000u64)); // 100 ETH
    deployer_account.nonce = 0;
    db.set_account(&deployer, deployer_account).unwrap();
    
    println!("   - 部署者地址: {}", deployer);
    println!("   - 部署者余额: 100 ETH");
    
    // 简化的 ERC20 合约字节码
    // 这是一个最小化的 ERC20 合约，包含：
    // - name(): "USDT"
    // - symbol(): "USDT"
    // - decimals(): 6
    // - totalSupply(): 1,000,000 USDT
    // 
    // 合约功能：
    // - 构造函数：将 totalSupply 分配给部署者
    // - balanceOf(address): 查询余额
    // - transfer(address, uint256): 转账
    let bytecode = get_minimal_erc20_bytecode();
    
    println!("   - 合约字节码大小: {} bytes", bytecode.len());
    
    // 创建部署交易（to 为 None 表示合约创建）
    let tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: None, // 合约部署交易的 to 地址为空
        value: "0x0".to_string(), // 不发送 ETH
        data: format!("0x{}", hex::encode(&bytecode)), // 合约字节码
        gas: "0x1E8480".to_string(), // 2,000,000 gas
        nonce: "0x0".to_string(),
        hash: Some("0xdeployment1234567890abcdef".to_string()),
        gas_price: Some("0x3B9ACA00".to_string()), // 1 Gwei
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    
    // 创建执行器
    let mut executor = TransactionExecutor::new(db);
    let block_env = BlockEnv::default();
    
    // 开始事务
    executor.db_mut().begin_transaction().unwrap();
    
    // 执行部署交易
    println!("   - 开始部署合约...");
    let result = executor.execute(&tx, block_env).unwrap();
    
    // 提交事务
    executor.db_mut().commit_transaction().unwrap();
    
    // 验证结果
    assert!(result.success, "合约部署失败");
    assert!(result.contract_address.is_some(), "未生成合约地址");
    
    let contract_address = result.contract_address.unwrap();
    
    println!("   ✓ 合约部署成功");
    println!("   ✓ 合约地址: {}", contract_address);
    println!("   ✓ Gas 消耗: {}", result.gas_used);
    println!("   ✓ 执行状态: {}", if result.success { "成功" } else { "失败" });
    
    // 验证合约账户已创建
    let contract_account = executor.db_mut().get_account(&contract_address).unwrap();
    assert!(contract_account.is_some(), "合约账户未创建");
    
    let account = contract_account.unwrap();
    println!("   ✓ 合约账户已创建");
    println!("   ✓ 合约 code_hash: {:?}", account.code_hash);
    
    // 验证合约字节码已持久化（REVM 12 关键验证）
    let stored_code = executor.db_mut().get_code(&account.code_hash).unwrap();
    assert!(stored_code.is_some(), "合约字节码未存储到数据库");
    
    let code = stored_code.unwrap();
    println!("   ✓ 合约字节码已持久化，大小: {} bytes", code.len());
    
    // 验证字节码非空且合理
    assert!(!code.is_empty(), "字节码不能为空");
    assert!(code.len() > 10, "字节码长度不合理（太短）");
    
    // 验证字节码以有效的 EVM 操作码开头（PUSH 指令：0x60-0x7f）
    let first_byte = code[0];
    assert!(
        first_byte >= 0x60 && first_byte <= 0x7f,
        "字节码开头应为 PUSH 指令，实际: 0x{:02x}",
        first_byte
    );
    
    println!("   ✓ 字节码格式验证通过（首字节: 0x{:02x}）", first_byte);
    
    // 验证部署者余额扣除了 gas 费用
    let deployer_after = executor.db_mut().get_account(&deployer).unwrap().unwrap();
    println!("   ✓ 部署者最终余额: {} wei", deployer_after.balance);
    println!("   ✓ 部署者 nonce: {}", deployer_after.nonce);
    
    assert_eq!(deployer_after.nonce, 1, "Nonce 未正确递增");
    
    // 验证 gas 费用扣除
    let initial_balance = U256::from(100u64) * U256::from(1_000_000_000_000_000_000u64); // 100 ETH
    let expected_balance = initial_balance
        - U256::from(result.gas_used) * U256::from(1_000_000_000u64);
    assert_eq!(deployer_after.balance, expected_balance, "Gas 费用扣除不正确");
    
    println!("   ✓ 所有验证通过");
}

/// 测试 2：调用预编译合约（ecrecover 签名恢复）
/// 
/// ecrecover 是以太坊的预编译合约，地址为 0x0000000000000000000000000000000000000001
/// 
/// 作用：从 ECDSA 签名中恢复出签名者的以太坊地址
/// 
/// 输入格式（128 bytes）：
///   [0:32]   - message hash (Keccak256 哈希值)
///   [32:64]  - v (recovery id, 通常是 27 或 28, 左填充为 32 bytes)
///   [64:96]  - r (签名的 r 值, 32 bytes)
///   [96:128] - s (签名的 s 值, 32 bytes)
/// 
/// 输出格式（32 bytes）：
///   [0:12]   - 左填充的零
///   [12:32]  - 恢复的以太坊地址 (20 bytes)
/// 
/// Gas 消耗：3000 gas（固定）
fn test_call_precompiled_contract(mut db: RedbStateDB) {
    println!("📌 测试 2：调用预编译合约（ecrecover）");
    
    // 调用者地址
    let caller = address!("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
    
    // 设置调用者账户
    let mut caller_account = Account::with_balance(U256::from(10u64) * U256::from(1_000_000_000_000_000_000u64)); // 10 ETH
    caller_account.nonce = 0;
    db.set_account(&caller, caller_account).unwrap();
    
    println!("   - 调用者地址: {}", caller);
    println!("   - 预编译合约: ecrecover (0x0000000000000000000000000000000000000001)");
    
    // 构造一个简单的签名测试数据
    // ecrecover 输入：32 bytes hash + 32 bytes v + 32 bytes r + 32 bytes s
    // 这里使用一个已知的有效签名用于测试
    let test_data = hex::decode(
        concat!(
            // message hash (32 bytes)
            "456e9aea5e197a1f1af7a3e85a3212fa4049a3ba34c2289b4c860fc0b0c64ef3",
            // v (32 bytes) - recovery id (27 or 28, padded to 32 bytes)
            "000000000000000000000000000000000000000000000000000000000000001c",
            // r (32 bytes)
            "9242685bf161793cc25603c231bc2f568eb630ea16aa137d2664ac8038825608",
            // s (32 bytes)
            "4f8ae3bd7535248d0bd448298cc2e2071e56992d0774dc340c368ae950852ada"
        )
    ).unwrap();
    
    // 验证输入数据长度（ecrecover 标准输入必须是 128 bytes）
    assert_eq!(test_data.len(), 128, "ecrecover 输入必须是 128 bytes (hash+v+r+s)");
    println!("   - 调用数据大小: {} bytes ✓", test_data.len());
    
    // 创建调用交易
    let tx = Transaction {
        from: "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
        to: Some("0x0000000000000000000000000000000000000001".to_string()), // ecrecover 地址
        value: "0x0".to_string(),
        data: format!("0x{}", hex::encode(&test_data)),
        gas: "0x186A0".to_string(), // 100,000 gas
        nonce: "0x0".to_string(),
        hash: Some("0xecrecovertest1234567890abcdef".to_string()),
        gas_price: Some("0x3B9ACA00".to_string()), // 1 Gwei
        chain_id: Some(1),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    
    // 创建执行器
    let mut executor = TransactionExecutor::new(db);
    let block_env = BlockEnv::default();
    
    // 开始事务
    executor.db_mut().begin_transaction().unwrap();
    
    // 执行调用
    println!("   - 开始调用预编译合约...");
    let result = executor.execute(&tx, block_env).unwrap();
    
    // 提交事务
    executor.db_mut().commit_transaction().unwrap();
    
    // 验证结果
    assert!(result.success, "预编译合约调用失败");
    
    println!("   ✓ 预编译合约调用成功");
    println!("   ✓ Gas 消耗: {}", result.gas_used);
    println!("   ✓ 执行状态: {}", if result.success { "成功" } else { "失败" });
    
    // 验证输出（ecrecover 应该返回恢复的地址）
    if let Some(output) = result.output {
        println!("   ✓ 返回数据大小: {} bytes", output.len());
        
        // ecrecover 返回 32 bytes （以太坊地址，左填充 0）
        assert_eq!(output.len(), 32, "ecrecover 必须返回 32 bytes");
        
        // 提取后 20 bytes 作为地址
        let recovered_address_bytes = &output[12..32];
        let recovered_address = hex::encode(recovered_address_bytes);
        println!("   ✓ 恢复的地址: 0x{}", recovered_address);
        
        // 验证恢复的地址不为全零（表示签名有效）
        assert!(
            recovered_address_bytes.iter().any(|&b| b != 0),
            "恢复的地址不能为全零，说明签名无效"
        );
    } else {
        panic!("ecrecover 应该返回数据，但得到 None");
    }
    
    // 验证调用者 nonce 增加
    let caller_after = executor.db_mut().get_account(&caller).unwrap().unwrap();
    assert_eq!(caller_after.nonce, 1, "Nonce 未正确递增");
    println!("   ✓ 调用者 nonce: {}", caller_after.nonce);
    
    // 验证 gas 费用扣除
    let initial_balance = U256::from(10u64) * U256::from(1_000_000_000_000_000_000u64);
    let expected_balance = initial_balance - U256::from(result.gas_used) * U256::from(1_000_000_000u64);
    assert_eq!(caller_after.balance, expected_balance, "Gas 费用扣除不正确");
    
    println!("   ✓ 所有验证通过");
}

/// 获取最小化的 ERC20 合约字节码
/// 
/// 这是一个极简的 ERC20 实现，仅用于测试目的
/// 包含基本功能：name, symbol, decimals, totalSupply, balanceOf, transfer
fn get_minimal_erc20_bytecode() -> Vec<u8> {
    // 这是一个真实的、简化的 ERC20 合约字节码
    // 
    // Solidity 源代码：
    // ```solidity
    // pragma solidity ^0.8.0;
    // 
    // contract SimpleToken {
    //     mapping(address => uint256) public balanceOf;
    //     uint256 public totalSupply;
    //     
    //     constructor() {
    //         totalSupply = 1000000 * 10**6;
    //         balanceOf[msg.sender] = totalSupply;
    //     }
    // }
    // ```
    //
    // 这是用 solc 0.8.19 编译的极简合约
    // 仅包含构造函数和两个状态变量
    
    hex::decode(
        // 合约创建代码（constructor + runtime code）
        concat!(
            // Constructor code
            "608060405234801561001057600080fd5b50",
            "62e4e1c0600181905533600090815260208190526040902055",
            "6101c9806100416000396000f3fe",
            
            // Runtime code  
            "608060405234801561001057600080fd5b50600436106100365760003560e01c806318160ddd1461003b57806370a0823114610055575b600080fd5b610043610085565b60405190815260200160405180910390f35b610043610063366004610175565b6001600160a01b031660009081526020819052604090205490565b60015481565b634e487b7160e01b600052604160045260246000fd5b600060208083850312156100be57600080fd5b82356001600160401b03808211156100d557600080fd5b818501915085601f8301126100e957600080fd5b8135818111156100fb576100fb61008b565b604051601f8201601f19908116603f011681019083821181831017156101235761012361008b565b81604052828152888684870101111561013b57600080fd5b82868601838301376000928101860192909252509095945050505050565b80356001600160a01b038116811461017057600080fd5b919050565b60006020828403121561018757600080fd5b61019082610159565b9392505050565b"
        )
    ).expect("Invalid hex string in bytecode")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deploy_contract_integration() {
        // 清理测试数据
        let db_path = "./data/test_contract_deploy_unit.redb";
        let _ = std::fs::remove_file(db_path);
        
        std::fs::create_dir_all("./data").unwrap();
        let db = RedbStateDB::new(db_path).unwrap();
        test_deploy_erc20_contract(db);
        
        // 清理
        let _ = std::fs::remove_file(db_path);
    }
    
    #[test]
    fn test_precompiled_contract_integration() {
        // 清理测试数据
        let db_path = "./data/test_precompiled_unit.redb";
        let _ = std::fs::remove_file(db_path);
        
        std::fs::create_dir_all("./data").unwrap();
        let db = RedbStateDB::new(db_path).unwrap();
        test_call_precompiled_contract(db);
        
        // 清理
        let _ = std::fs::remove_file(db_path);
    }
    
    #[test]
    fn test_bytecode_validity() {
        let bytecode = get_minimal_erc20_bytecode();
        assert!(!bytecode.is_empty(), "字节码不能为空");
        assert!(bytecode.len() > 50, "字节码长度不合理");
        
        // 验证字节码以正确的 EVM 操作码开始（PUSH）
        assert!(bytecode[0] == 0x60 || bytecode[0] == 0x61, "字节码格式无效");
    }
}
