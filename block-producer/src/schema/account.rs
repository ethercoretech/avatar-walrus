//! 账户数据结构
//! 
//! 定义以太坊账户的核心字段

use alloy_primitives::{U256, B256};
use serde::{Deserialize, Serialize};

/// 以太坊账户
/// 
/// 包含账户的核心状态：余额、nonce、存储根、代码哈希
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// 交易计数器（防止重放攻击）
    pub nonce: u64,
    
    /// 账户余额（以 wei 为单位）
    pub balance: U256,
    
    /// 存储树的根哈希（合约账户）
    pub storage_root: B256,
    
    /// 代码哈希（合约账户）
    /// 对于 EOA（外部账户），code_hash 为空哈希
    pub code_hash: B256,
}

impl Account {
    /// 创建空账户
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 创建指定余额的账户
    pub fn with_balance(balance: U256) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }
    
    /// 检查是否为合约账户
    pub fn is_contract(&self) -> bool {
        self.code_hash != EMPTY_CODE_HASH
    }
    
    /// 检查是否为空账户
    pub fn is_empty(&self) -> bool {
        self.nonce == 0 
            && self.balance == U256::ZERO 
            && self.code_hash == EMPTY_CODE_HASH
    }
    
    /// 增加 nonce
    pub fn increment_nonce(&mut self) {
        self.nonce += 1;
    }
    
    /// 增加余额
    pub fn add_balance(&mut self, amount: U256) {
        self.balance += amount;
    }
    
    /// 减少余额（会检查是否充足）
    pub fn sub_balance(&mut self, amount: U256) -> Result<(), &'static str> {
        if self.balance < amount {
            return Err("Insufficient balance");
        }
        self.balance -= amount;
        Ok(())
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            nonce: 0,
            balance: U256::ZERO,
            storage_root: EMPTY_STORAGE_ROOT,
            code_hash: EMPTY_CODE_HASH,
        }
    }
}

/// 空存储根（空 Merkle Patricia Trie 的根哈希）
pub const EMPTY_STORAGE_ROOT: B256 = B256::new([
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
]);

/// 空代码哈希（keccak256("")）
pub const EMPTY_CODE_HASH: B256 = B256::new([
    0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c,
    0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
    0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b,
    0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
]);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_account_default() {
        let acc = Account::default();
        assert_eq!(acc.nonce, 0);
        assert_eq!(acc.balance, U256::ZERO);
        assert!(acc.is_empty());
        assert!(!acc.is_contract());
    }
    
    #[test]
    fn test_account_balance() {
        let mut acc = Account::with_balance(U256::from(1000));
        assert_eq!(acc.balance, U256::from(1000));
        
        acc.add_balance(U256::from(500));
        assert_eq!(acc.balance, U256::from(1500));
        
        acc.sub_balance(U256::from(300)).unwrap();
        assert_eq!(acc.balance, U256::from(1200));
        
        assert!(acc.sub_balance(U256::from(2000)).is_err());
    }
    
    #[test]
    fn test_account_nonce() {
        let mut acc = Account::default();
        assert_eq!(acc.nonce, 0);
        
        acc.increment_nonce();
        assert_eq!(acc.nonce, 1);
        
        acc.increment_nonce();
        assert_eq!(acc.nonce, 2);
    }
}
