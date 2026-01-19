//! 存储槽数据结构
//! 
//! 定义合约存储的键值对

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

/// 存储槽
/// 
/// 表示合约存储中的一个键值对
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageSlot {
    /// 合约地址
    pub address: Address,
    
    /// 存储槽键（256 位）
    pub key: U256,
    
    /// 存储槽值（256 位）
    pub value: U256,
}

impl StorageSlot {
    /// 创建新的存储槽
    pub fn new(address: Address, key: U256, value: U256) -> Self {
        Self {
            address,
            key,
            value,
        }
    }
    
    /// 检查是否为零值槽位（用于 gas 优化）
    pub fn is_zero(&self) -> bool {
        self.value == U256::ZERO
    }
    
    /// 计算存储槽的哈希键（用于 MPT）
    pub fn hashed_key(&self) -> [u8; 32] {
        use alloy_primitives::keccak256;
        let key_bytes = self.key.to_be_bytes::<32>();
        keccak256(key_bytes).into()
    }
}

/// 存储变更
/// 
/// 记录存储槽的变更（原值 -> 新值）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageChange {
    pub address: Address,
    pub key: U256,
    pub old_value: U256,
    pub new_value: U256,
}

impl StorageChange {
    pub fn new(address: Address, key: U256, old_value: U256, new_value: U256) -> Self {
        Self {
            address,
            key,
            old_value,
            new_value,
        }
    }
    
    /// 检查是否真的发生了变更
    pub fn is_changed(&self) -> bool {
        self.old_value != self.new_value
    }
    
    /// 计算 gas 退款（SSTORE 操作）
    pub fn gas_refund(&self) -> i64 {
        match (self.old_value == U256::ZERO, self.new_value == U256::ZERO) {
            (false, true) => 15000,  // 清除存储
            (true, false) => -20000, // 设置新存储
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    
    #[test]
    fn test_storage_slot() {
        let addr = address!("0000000000000000000000000000000000000001");
        let slot = StorageSlot::new(addr, U256::from(1), U256::from(100));
        
        assert_eq!(slot.address, addr);
        assert_eq!(slot.key, U256::from(1));
        assert_eq!(slot.value, U256::from(100));
        assert!(!slot.is_zero());
        
        let zero_slot = StorageSlot::new(addr, U256::from(2), U256::ZERO);
        assert!(zero_slot.is_zero());
    }
    
    #[test]
    fn test_storage_change() {
        let addr = address!("0000000000000000000000000000000000000001");
        
        // 设置新存储
        let change1 = StorageChange::new(addr, U256::from(1), U256::ZERO, U256::from(100));
        assert!(change1.is_changed());
        assert_eq!(change1.gas_refund(), -20000);
        
        // 清除存储
        let change2 = StorageChange::new(addr, U256::from(1), U256::from(100), U256::ZERO);
        assert!(change2.is_changed());
        assert_eq!(change2.gas_refund(), 15000);
        
        // 修改存储
        let change3 = StorageChange::new(addr, U256::from(1), U256::from(100), U256::from(200));
        assert!(change3.is_changed());
        assert_eq!(change3.gas_refund(), 0);
        
        // 无变更
        let change4 = StorageChange::new(addr, U256::from(1), U256::from(100), U256::from(100));
        assert!(!change4.is_changed());
    }
}
